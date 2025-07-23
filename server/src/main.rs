use anyhow::Result;
use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;
use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::any,
};
use axum_extra::TypedHeader;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

//allows to split the websocket stream into separate TX and RX branches
use futures_util::stream::StreamExt;

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
            .route("/interactive-shell", any(ws_handler))
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    // Retrieve user-agent of the connection from the headers
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown user-agent")
    };

    info!(
        user_agent = %user_agent,
        addr = %addr,
        "New connection to websocket!",
    );

    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, who: SocketAddr) {
    let (mut _sender, mut receiver) = socket.split();

    let _ = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            if process_ws_message(message, who).is_break() {
                break;
            }
        }
    });
}

fn process_ws_message(message: Message, who: SocketAddr) -> ControlFlow<(), ()> {
    match message {
        Message::Text(t) => {
            info!(who = %who, text = %t.as_str(), "Received message!");
        }
        Message::Close(c) => {
            if let Some(control_flow) = c {
                info!(who = %who, code = %control_flow.code, reason = %control_flow.reason, "Received close message");
                return ControlFlow::Break(());
            }
        }
        _ => {
            error!(who = %who, "Received unsupported message");
        }
    }
    ControlFlow::Continue(())
}
