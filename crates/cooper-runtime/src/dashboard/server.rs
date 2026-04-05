use axum::response::Html;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use tokio::net::TcpListener;

use super::html;

/// Start the Cooper dashboard. Tries the preferred port, then falls
/// back to nearby ports. Never blocks or fails the main server.
pub async fn start(preferred_port: u16, api_port: u16) {
    let router = Router::new().route("/", get({
        let port = api_port;
        move || async move { Html(html::render(port)) }
    }));

    // Try preferred port, then +1, +2, etc.
    for offset in 0..10u16 {
        let port = preferred_port + offset;
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                if offset > 0 {
                    tracing::info!("Dashboard on port {} (preferred {} was taken)", port, preferred_port);
                }
                let _ = axum::serve(listener, router).await;
                return;
            }
            Err(_) => continue,
        }
    }

    tracing::warn!("Dashboard: no free port found near {}", preferred_port);
}
