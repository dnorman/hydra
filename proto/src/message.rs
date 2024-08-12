use crate::event::ingress::{FetchIngressLogsRequest, FetchIngressLogsResponse};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub enum Message {
    Request(Request),
    Response(Response),
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub struct Request {
    pub id: usize,
    pub payload: RequestPayload,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub enum RequestPayload {
    FetchIngressLogs(FetchIngressLogsRequest),
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub request_id: usize,
    pub payload: ResponsePayload,
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub enum ResponsePayload {
    FetchIngressLogs(FetchIngressLogsResponse),
    Error(String),
}
