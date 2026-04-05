use crate::error::{CooperError, ErrorCode};
use crate::js::JsRuntime;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use cooper_codegen::analyzer::{ProjectAnalysis, RouteInfo};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub analysis: RwLock<ProjectAnalysis>,
    pub js_runtime: RwLock<JsRuntime>,
    pub project_root: std::path::PathBuf,
}

/// Build an axum Router from the analyzed project
pub fn build_router(state: Arc<AppState>, analysis: &ProjectAnalysis) -> Router {
    let mut router = Router::new();

    for route in &analysis.routes {
        let axum_path = cooper_path_to_axum(&route.path);
        let route_info = route.clone();

        let handler = {
            let state = Arc::clone(&state);
            let info = route_info.clone();
            move |path_params: Option<Path<HashMap<String, String>>>,
                  headers: HeaderMap,
                  req: Request<Body>| {
                let state = Arc::clone(&state);
                let info = info.clone();
                async move { handle_request(state, info, path_params, headers, req).await }
            }
        };

        router = match route.method.as_str() {
            "GET" => router.route(&axum_path, get(handler)),
            "POST" => router.route(&axum_path, post(handler)),
            "PUT" => router.route(&axum_path, put(handler)),
            "PATCH" => router.route(&axum_path, patch(handler)),
            "DELETE" => router.route(&axum_path, delete(handler)),
            _ => {
                tracing::warn!("Unsupported method: {}", route.method);
                router
            }
        };
    }

    // Health check endpoint (always present)
    router = router.route(
        "/_cooper/health",
        get(|| async { (StatusCode::OK, r#"{"status":"ok"}"#) }),
    );

    // Info endpoint — returns analyzed routes
    let info_state = Arc::clone(&state);
    router = router.route(
        "/_cooper/info",
        get(move || {
            let state = Arc::clone(&info_state);
            async move {
                let analysis = state.analysis.read().await;
                let info = serde_json::json!({
                    "routes": analysis.routes.iter().map(|r| {
                        serde_json::json!({
                            "method": r.method,
                            "path": r.path,
                            "auth": r.auth,
                            "handler": r.export_name,
                            "source": r.source_file,
                        })
                    }).collect::<Vec<_>>(),
                    "databases": analysis.databases.iter().map(|d| {
                        serde_json::json!({ "name": d.name, "engine": d.engine })
                    }).collect::<Vec<_>>(),
                    "topics": analysis.topics.iter().map(|t| {
                        serde_json::json!({ "name": t.name })
                    }).collect::<Vec<_>>(),
                    "crons": analysis.crons.iter().map(|c| {
                        serde_json::json!({ "name": c.name, "schedule": c.schedule })
                    }).collect::<Vec<_>>(),
                    "queues": analysis.queues.iter().map(|q| {
                        serde_json::json!({ "name": q.name })
                    }).collect::<Vec<_>>(),
                    "pages": analysis.pages.iter().map(|p| {
                        serde_json::json!({ "route": p.route, "source": p.source_file })
                    }).collect::<Vec<_>>(),
                });
                (
                    StatusCode::OK,
                    [("content-type", "application/json")],
                    serde_json::to_string_pretty(&info).unwrap(),
                )
            }
        }),
    );

    router.with_state(())
}

async fn handle_request(
    state: Arc<AppState>,
    route: RouteInfo,
    path_params: Option<Path<HashMap<String, String>>>,
    headers: HeaderMap,
    req: Request<Body>,
) -> Response {
    let params = path_params.map(|Path(p)| p).unwrap_or_default();

    // Extract auth token from Authorization header
    let auth_token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    // Collect headers into a HashMap for the JS bridge
    let header_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str().ok().map(|val| (k.as_str().to_string(), val.to_string()))
        })
        .collect();

    // Read the request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return CooperError::new(ErrorCode::InvalidArgument, format!("Bad request body: {e}"))
                .into_response();
        }
    };

    let body: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(_) => Value::Null,
        }
    };

    // Build the handler input — merge path params into body
    let mut input = match body {
        Value::Object(map) => Value::Object(map),
        _ => Value::Object(serde_json::Map::new()),
    };

    if let Value::Object(ref mut map) = input {
        for (key, value) in params {
            map.insert(key, Value::String(value));
        }
    }

    // Execute via JS runtime
    let runtime = state.js_runtime.read().await;
    match runtime
        .call_handler_with_auth(
            &route.source_file,
            &route.export_name,
            &input,
            auth_token.as_deref(),
            Some(&header_map),
        )
        .await
    {
        Ok(result) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            result,
        )
            .into_response(),
        Err(e) => {
            let err_str = e.to_string();
            // Try to parse as a Cooper structured error
            if let Ok(parsed) = serde_json::from_str::<Value>(&err_str) {
                if let Some(error_obj) = parsed.get("error") {
                    let code = error_obj
                        .get("code")
                        .and_then(|c| c.as_str())
                        .unwrap_or("INTERNAL");
                    let status = match code {
                        "NOT_FOUND" => StatusCode::NOT_FOUND,
                        "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
                        "PERMISSION_DENIED" => StatusCode::FORBIDDEN,
                        "RATE_LIMITED" => StatusCode::TOO_MANY_REQUESTS,
                        "INVALID_ARGUMENT" => StatusCode::BAD_REQUEST,
                        "VALIDATION_FAILED" => StatusCode::UNPROCESSABLE_ENTITY,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    return (
                        status,
                        [("content-type", "application/json")],
                        err_str,
                    )
                        .into_response();
                }
            }
            CooperError::new(ErrorCode::Internal, format!("Handler error: {e}")).into_response()
        }
    }
}

/// Convert Cooper-style paths (/users/:id) to Axum-style (/users/{id})
fn cooper_path_to_axum(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if let Some(param) = segment.strip_prefix(':') {
                format!("{{{param}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_conversion() {
        assert_eq!(cooper_path_to_axum("/users/:id"), "/users/{id}");
        assert_eq!(
            cooper_path_to_axum("/users/:userId/posts/:postId"),
            "/users/{userId}/posts/{postId}"
        );
        assert_eq!(cooper_path_to_axum("/health"), "/health");
    }
}
