use crate::dashboard;
use crate::js::JsRuntime;
use crate::router::{build_router, AppState};
use anyhow::Result;
use cooper_codegen::analyzer::ProjectAnalysis;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

const DASHBOARD_PORT: u16 = 9500;

pub struct RuntimeServer {
    port: u16,
    state: Arc<AppState>,
}

impl RuntimeServer {
    pub fn new(port: u16, project_root: PathBuf, analysis: ProjectAnalysis) -> Self {
        let js_runtime = JsRuntime::new(project_root.clone());
        Self {
            port,
            state: Arc::new(AppState {
                analysis: RwLock::new(analysis),
                js_runtime: RwLock::new(js_runtime),
                project_root,
            }),
        }
    }

    pub async fn start(&self) -> Result<()> {
        // Start JS worker pool
        {
            let mut runtime = self.state.js_runtime.write().await;
            if let Err(e) = runtime.start().await {
                tracing::warn!("JS workers not started: {e} — handlers will return debug responses");
            }
        }

        let analysis = self.state.analysis.read().await;
        let router = build_router(Arc::clone(&self.state), &analysis)
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());
        drop(analysis);

        // Start dashboard on a background task (non-fatal if port is taken)
        let api_port = self.port;
        tokio::spawn(async move {
            dashboard::server::start(DASHBOARD_PORT, api_port).await;
        });

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }

    pub async fn reload(&self) -> Result<()> {
        let new_analysis =
            cooper_codegen::analyzer::analyze(&self.state.project_root)?;

        let mut analysis = self.state.analysis.write().await;
        *analysis = new_analysis;

        let runtime = self.state.js_runtime.read().await;
        runtime.invalidate().await?;

        Ok(())
    }
}
