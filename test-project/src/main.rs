//! Test Project - A sample Rust application for expresso-kit validation
//!
//! This project demonstrates a typical Rust web application setup with:
//! - PostgreSQL database with SQLx
//! - Redis caching
//! - Environment configuration
//! - Health checks

use std::net::SocketAddr;

use axum::{Router, routing::get, Json};
use serde::Serialize;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    app_name: String,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        app_name: std::env::var("APP_NAME").unwrap_or_else(|_| "test-project".to_string()),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get configuration from environment
    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("APP_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("APP_PORT must be a valid port number");

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/", get(|| async { "Hello from test-project!" }));

    // Start server
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
