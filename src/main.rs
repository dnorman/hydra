mod appstate;
mod error;
mod fetch;
// mod ingress;
mod storage;

use appstate::AppState;

use anyhow::Result;
use axum::{
    http::StatusCode,
    routing::{get, post},
    Router,
};

#[tokio::main]
async fn main() -> Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();
    let state = AppState::new()?;

    // build our application with a route
    let app = Router::new()
        .route("/", get(root))
        // .route("/ingress", post(ingress::capture))
        // .route("/ingress", get(ingress::list))
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
