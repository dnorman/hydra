use axum::{
    extract::{Host, Path, Query, State},
    http::{HeaderMap, Method},
    response::IntoResponse,
    Json,
};
use bytes::Bytes;
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

// list the latest 10 documents in the ingress column family
pub async fn list(
    state: State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let tree = state.storage.get_handle("ingress")?;
    let after_key = params.get("after_key").map(|k| URL_SAFE.decode(k).unwrap());

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<body>
<h1>Latest 10 Documents in the Ingress Log</h1>
<ol>"#,
    );

    let mut items = Vec::new();
    let iter = match after_key {
        Some(key) => tree.range(..key).rev(),
        None => tree.iter().rev(),
    };

    for item in iter.take(11) {
        let (key, _) = item?;
        items.push(key);
    }

    for key in items.iter().take(10) {
        let encoded_key = URL_SAFE.encode(key);
        html.push_str(&format!("<li>{encoded_key}</li>"));
    }

    html.push_str("</ol>");

    if items.len() > 10 {
        let last_key = URL_SAFE.encode(&items[9]);
        html.push_str(&format!(
            r#"<a href="?after_key={}">Next page</a>"#,
            last_key
        ));
    }

    html.push_str("</body></html>");

    Ok(axum::response::Html(html))
}
