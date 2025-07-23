mod websocket;

use anyhow::Result;
use std::net::SocketAddr;

use axum::{Router, routing::any};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let http_server_config = HttpServerConfig::new("127.0.0.1".into(), 3000);
    let http_server = HttpServer::new(http_server_config);
    http_server.run().await
}

struct HttpServerConfig {
    ip: String,
    port: usize,
}

impl HttpServerConfig {
    pub fn new(ip: String, port: usize) -> Self {
        Self { ip, port }
    }
}

struct HttpServer {
    config: HttpServerConfig,
}

impl HttpServer {
    pub fn new(config: HttpServerConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<()> {
        // Setting up logging mechanism
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
                }),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        // Creating application with routes
        let app = Router::new()
            .route("/agent/it-shell", any(crate::websocket::agent::ws_handle))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::default().include_headers(false)),
            );

        // Listening to the TCP Port
        let listener =
            tokio::net::TcpListener::bind(format!("{}:{}", &self.config.ip, self.config.port))
                .await
                .unwrap();

        // Running application with the TCP listener
        tracing::debug!(
            "HTTP Server listening on: {}",
            listener.local_addr().unwrap()
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| e.into())
    }
}
