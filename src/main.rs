mod error;
mod ingress;
mod storage;
use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use axum::{
    http::StatusCode,
    routing::{get, post},
    Router,
};

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);
pub struct AppStateInner {
    pub storage: storage::StorageEngine,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let storage = storage::StorageEngine::new()?;
        Ok(Self(Arc::new(AppStateInner { storage })))
    }
}

impl Deref for AppState {
    type Target = Arc<AppStateInner>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();
    let state = AppState::new()?;

    // build our application with a route
    let app = Router::new()
        .route("/", get(root))
        .route("/ingress", post(ingress::capture))
        .route("/ingress", get(ingress::list))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    eprintln!("Server running on http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

pub async fn root() -> Result<String, StatusCode> {
    Ok("Hello, world!".to_string())
}
