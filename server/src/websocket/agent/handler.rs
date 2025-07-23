use std::net::SocketAddr;
use std::ops::ControlFlow;

use axum::extract::connect_info::ConnectInfo;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};
use axum_extra::TypedHeader;
use tracing::{error, info};

//allows to split the websocket stream into separate TX and RX branches
use futures_util::stream::StreamExt;

pub async fn ws_handle(
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
            if process_message(message, who).is_break() {
                break;
            }
        }
    })
    .await;
}

fn process_message(message: Message, who: SocketAddr) -> ControlFlow<(), ()> {
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
