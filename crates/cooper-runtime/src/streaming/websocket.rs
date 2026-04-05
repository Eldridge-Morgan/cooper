use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::js::JsRuntime;
use crate::router::AppState;

/// Handle a WebSocket upgrade for a Cooper route with `stream: "websocket"`.
pub fn ws_upgrade(
    ws: WebSocketUpgrade,
    js_runtime: Arc<RwLock<JsRuntime>>,
    source_file: String,
    export_name: String,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, js_runtime, source_file, export_name))
}

async fn handle_ws(
    mut socket: WebSocket,
    js_runtime: Arc<RwLock<JsRuntime>>,
    source_file: String,
    export_name: String,
) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                let input: Value =
                    serde_json::from_str(&text).unwrap_or(Value::String(text.to_string()));

                let rt = js_runtime.read().await;
                let call_input = serde_json::json!({ "data": input, "type": "message" });

                match rt.call_handler(&source_file, &export_name, &call_input).await {
                    Ok(response) => {
                        if socket.send(Message::Text(response.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let err = serde_json::json!({"error": e.to_string()});
                        if socket
                            .send(Message::Text(err.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
