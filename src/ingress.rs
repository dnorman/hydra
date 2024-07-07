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

use crate::{error::AppError, AppState};

#[derive(Serialize, Deserialize)]
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

    let handle = state.storage.get_handle("ingress")?;
    handle.insert(key, bincode::serialize(&log)?)?;

    Ok(Json(IngressResponse { event_id }))
}

use base64::{engine::general_purpose::URL_SAFE, Engine as _};

pub async fn list(
    state: State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let tree = state.storage.get_handle("ingress")?;
    let earlier_than = params
        .get("earlier_than")
        .map(|k| URL_SAFE.decode(k).unwrap());
    let later_than = params
        .get("later_than")
        .map(|k| URL_SAFE.decode(k).unwrap());
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(10);
    let fetch_limit = limit + 1; // Fetch one extra to determine if there's a next page

    let (items, has_earlier_page, has_later_page) = match (earlier_than, later_than) {
        (Some(end), None) => {
            let mut vec: Vec<_> = tree
                .range(..end)
                .rev()
                .take(fetch_limit)
                .map(|item| item.unwrap())
                .collect();
            let has_earlier = vec.len() > limit;
            vec.truncate(limit);
            vec.reverse();
            (vec, has_earlier, true)
        }
        (None, Some(start)) => {
            let vec: Vec<_> = tree
                .range(start..)
                .take(fetch_limit)
                .map(|item| item.unwrap())
                .collect();
            let has_later = vec.len() > limit;
            (vec[..limit.min(vec.len())].to_vec(), false, has_later)
        }
        (None, None) => {
            let vec: Vec<_> = tree
                .iter()
                .rev()
                .take(fetch_limit)
                .map(|item| item.unwrap())
                .collect();
            let has_earlier = vec.len() > limit;
            (vec[..limit.min(vec.len())].to_vec(), has_earlier, false)
        }
        _ => return Err(anyhow!("Cannot specify both earlier_than and later_than").into()),
    };

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body {
            font-family: Arial, sans-serif;
        }
        .container {
            margin: 0 auto;
            padding: 20px;
        }
        .navigation {
            margin-bottom: 20px;
        }
        .navigation a {
            margin-right: 10px;
        }
        .table-container {
            max-height: 600px;
            overflow-y: auto;
        }
        table {
            width: 100%;
            border-collapse: collapse;
        }
        th, td {
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
        }
        th {
            background-color: #f2f2f2;
            position: sticky;
            top: 0;
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

    if has_earlier_page {
        let first_key = URL_SAFE.encode(&items.first().unwrap().0);
        html.push_str(&format!(
            r#"<a href="?earlier_than={}&limit={}">Earlier events</a>"#,
            first_key, limit
        ));
    }

    if has_later_page {
        let last_key = URL_SAFE.encode(&items.last().unwrap().0);
        html.push_str(&format!(
            r#"<a href="?later_than={}&limit={}">Later events</a>"#,
            last_key, limit
        ));
    }

    html.push_str(
        r#"</div>
    <div class="table-container">
    <table>
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
    </tr>"#,
    );

    for (key, value) in &items {
        let encoded_key = URL_SAFE.encode(key);
        let log: IngressLog = bincode::deserialize(value)?;
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

    html.push_str("</table></div></div></body></html>");

    Ok(axum::response::Html(html))
}
