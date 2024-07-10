use anyhow::anyhow;
use axum::{
    extract::{Host, Path, Query, State},
    http::{HeaderMap, Method},
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
use sled::IVec;
use std::{collections::HashMap, net::SocketAddr};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::{
    error::AppError,
    fetch::{fetch, FetchQuery, FetchResult, Order, Record},
    AppState,
};

#[derive(Serialize, Deserialize, Clone)]
struct IngressLog {
    event_id: Ulid,
    date: chrono::DateTime<chrono::Utc>,
    remote_addr: Option<SocketAddr>,
    method: String,
    host: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Bytes,
}
impl Record for IngressLog {
    type ID = Ulid;
    fn id(&self) -> &Self::ID {
        &self.event_id
    }
}

#[derive(Serialize, Deserialize)]
struct IngressResponse {
    event_id: Ulid,
}

pub async fn capture(
    state: State<AppState>,
    // uncommenting these causes an error
    // remote_addr: Option<SocketAddr>,
    method: Method,
    Host(host): Host,
    path: Path<Vec<String>>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let event_id = ulid::Ulid::new();
    let key = format!("test|{}", event_id);

    println!("Ingress request: {:?}", event_id);

    let log = IngressLog {
        event_id,
        remote_addr: None,
        method: method.to_string(),
        host,
        path: path.join("/").to_string(),
        query,
        date: chrono::Utc::now(),
        body,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
            .collect(),
    };

    let handle = state.storage.subtree("ingress")?;
    handle.insert(key, bincode::serialize(&log)?)?;

    Ok(Json(IngressResponse { event_id }))
}

use base64::{engine::general_purpose::URL_SAFE, Engine as _};

#[derive(PartialEq, Eq)]
enum Mode {
    Preceding,
    Following,
    Default,
}

pub async fn list(
    state: State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let tree = state.storage.subtree("ingress")?;

    let mut query = FetchQuery::new();

    let (cursor, order, mode) = if let Some(following) = params.get("following") {
        (
            Some(URL_SAFE.decode(following)?),
            Order::Descending,
            Mode::Following,
        )
    } else if let Some(preceding) = params.get("preceding") {
        (
            Some(URL_SAFE.decode(preceding)?),
            Order::Ascending,
            Mode::Preceding,
        )
    } else {
        (None, Order::Descending, Mode::Default)
    };

    if let Some(cursor) = cursor {
        query = query.cursor(cursor);
    }
    query = query.order(order);

    if let Some(limit) = params.get("limit").and_then(|s| s.parse().ok()) {
        query = query.limit(limit);
    }

    let fetch_result = fetch::<IngressLog, _>(&tree, query)?;

    render_ingress_logs_html(&fetch_result, params.get("limit"), Order::Descending, mode)
}

fn render_ingress_logs_html(
    fetch_result: &FetchResult<IngressLog>,
    limit_param: Option<&String>,
    display_order: Order,
    mode: Mode,
) -> Result<impl IntoResponse, AppError> {
    let limit = limit_param
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10);

    let mut items = fetch_result.items.clone();
    if fetch_result.order != display_order {
        items.reverse();
    }

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body, html {
            height: 100%;
            margin: 0;
            font-family: Arial, sans-serif;
        }
        .container {
            display: flex;
            flex-direction: column;
            height: 100%;
            padding: 20px;
            box-sizing: border-box;
        }
        .navigation {
            margin-bottom: 20px;
        }
        .navigation a {
            margin-right: 10px;
        }
        .table-container {
            flex: 1;
            overflow: auto;
        }
        table {
            width: 100%;
            border-collapse: separate;
            border-spacing: 0;
        }
        th, td {
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
        }
        thead {
            position: sticky;
            top: 0;
            background-color: #f2f2f2;
            z-index: 1;
        }
        th {
            background-color: #f2f2f2;
        }
        pre {
            white-space: pre-wrap;
            word-wrap: break-word;
        }
    </style>
</head>
<body>
<div class="container">
    <h1>Documents in the Ingress Log</h1>
    <div class="navigation">"#,
    );

    if items.is_empty() {
        // todo use the present cursor but reverse the direction and present the previous/next page link
        // in theory this shouldn't happen often, but could be possible if the item is deleted
    } else {
        // compare the first and last key to get the least and greatest key
        let first_key = &items.first().unwrap().0;
        let last_key = &items.last().unwrap().0;
        let least_key = URL_SAFE.encode(first_key.min(last_key));
        let greatest_key = URL_SAFE.encode(first_key.max(last_key));

        // an instruction to show items preceeding a given cursor
        // necessitates at least one greater key (the cursor).
        let mut has_following = mode == Mode::Preceding;

        // an instruction to show items following a given cursor
        // necessitates at least one lesser key (the cursor).
        let mut has_preceding = mode == Mode::Following;

        if fetch_result.more_records {
            if fetch_result.order == Order::Ascending {
                has_following = true;
            } else {
                has_preceding = true;
            }
        }
        // not quite right, but this is close
        if fetch_result.order == display_order {
            if has_preceding {
                html.push_str(&format!(
                    r#"<a href="?preceding={least_key}&limit={limit}">Previous page</a>"#
                ));
            }
            if has_following {
                html.push_str(&format!(
                    r#"<a href="?following={greatest_key}&limit={limit}">Next page</a>"#
                ));
            }
        } else {
            if has_following {
                html.push_str(&format!(
                    r#"<a href="?following={greatest_key}&limit={limit}">Previous page</a>"#
                ));
            }
            if has_preceding {
                html.push_str(&format!(
                    r#"<a href="?preceding={least_key}&limit={limit}">Next page</a>"#
                ));
            }
        }
    }

    html.push_str(
        r#"</div>
    <div class="table-container">
    <table>
    <thead>
    <tr>
        <th>Event ID</th>
        <th>Date</th>
        <th>Remote Addr</th>
        <th>Method</th>
        <th>Host</th>
        <th>Path</th>
        <th>Query</th>
        <th>Headers</th>
        <th>Body</th>
    </tr>
    </thead>
    <tbody>"#,
    );

    for (key, log) in &items {
        let encoded_key = URL_SAFE.encode(key);
        let body_utf8 = String::from_utf8_lossy(&log.body);
        html.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td><pre>{}</pre></td>
                <td><pre>{}</pre></td>
                <td><pre>{}</pre></td>
            </tr>"#,
            log.event_id,
            log.date,
            log.remote_addr
                .map_or("N/A".to_string(), |addr| addr.to_string()),
            html_escape::encode_text(&log.method),
            html_escape::encode_text(&log.host),
            html_escape::encode_text(&log.path),
            html_escape::encode_text(&serde_json::to_string_pretty(&log.query)?),
            html_escape::encode_text(&serde_json::to_string_pretty(&log.headers)?),
            html_escape::encode_text(&body_utf8),
        ));
    }

    html.push_str("</tbody></table></div></div></body></html>");

    Ok(axum::response::Html(html))
}
