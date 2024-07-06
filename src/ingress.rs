use axum::{
    extract::{Host, Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
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
    id: u64,
}

pub async fn capture(
    state: State<AppState>,
    // uncommenting these causes an error
    // remote_addr: Option<SocketAddr>,
    method: Method,
    Host(host): Host,
    path: Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    let event_id = ulid::Ulid::new();
    let key = format!("test|{}", event_id);

    let log = IngressLog {
        event_id,
        remote_addr: None,
        method: method.to_string(),
        host,
        path: path.to_string(),
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

    Ok(Json(IngressResponse { id: 1 }))
}

use base64::{engine::general_purpose::URL_SAFE, Engine as _};

// list the latest 10 documents in the ingress column family
pub async fn list(state: State<AppState>) -> Result<impl IntoResponse, AppError> {
    let tree = state.storage.get_handle("ingress")?;

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<body>
<h1>Latest 10 Documents in the Ingress Log</h1>
<ol>"#,
    );

    for item in tree.iter().rev().take(10) {
        let (key, _) = item?;
        let encoded_key = URL_SAFE.encode(&key);
        html.push_str(&format!("<li>{encoded_key}</li>"));
    }

    html.push_str("</ol></body></html>");

    Ok(axum::response::Html(html))
}
