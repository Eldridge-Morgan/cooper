use crate::error::{CooperError, ErrorCode};
use crate::js::JsRuntime;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
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

        match route.method.as_str() {
            "GET" => {
                router = router.route(
                    &axum_path,
                    get({
                        let state = Arc::clone(&state);
                        let info = route_info.clone();
                        move |path_params: Option<Path<HashMap<String, String>>>,
                              req: Request<Body>| {
                            handle_request(state, info, path_params, req)
                        }
                    }),
                );
            }
            "POST" => {
                router = router.route(
                    &axum_path,
                    post({
                        let state = Arc::clone(&state);
                        let info = route_info.clone();
                        move |path_params: Option<Path<HashMap<String, String>>>,
                              req: Request<Body>| {
                            handle_request(state, info, path_params, req)
                        }
                    }),
                );
            }
            "PUT" => {
                router = router.route(
                    &axum_path,
                    put({
                        let state = Arc::clone(&state);
                        let info = route_info.clone();
                        move |path_params: Option<Path<HashMap<String, String>>>,
                              req: Request<Body>| {
                            handle_request(state, info, path_params, req)
                        }
                    }),
                );
            }
            "PATCH" => {
                router = router.route(
                    &axum_path,
                    patch({
                        let state = Arc::clone(&state);
                        let info = route_info.clone();
                        move |path_params: Option<Path<HashMap<String, String>>>,
                              req: Request<Body>| {
                            handle_request(state, info, path_params, req)
                        }
                    }),
                );
            }
            "DELETE" => {
                router = router.route(
                    &axum_path,
                    delete({
                        let state = Arc::clone(&state);
                        let info = route_info.clone();
                        move |path_params: Option<Path<HashMap<String, String>>>,
                              req: Request<Body>| {
                            handle_request(state, info, path_params, req)
                        }
                    }),
                );
            }
            _ => {
                tracing::warn!("Unsupported method: {}", route.method);
            }
        }
    }

    router.with_state(())
}

async fn handle_request(
    state: Arc<AppState>,
    route: RouteInfo,
    path_params: Option<Path<HashMap<String, String>>>,
    req: Request<Body>,
) -> Response {
    let params = path_params
        .map(|Path(p)| p)
        .unwrap_or_default();

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
        .call_handler(&route.source_file, &route.export_name, &input)
        .await
    {
        Ok(result) => (StatusCode::OK, [("content-type", "application/json")], result).into_response(),
        Err(e) => {
            // Try to parse as a CooperError from JS
            if let Ok(err) = serde_json::from_str::<CooperError>(&e.to_string()) {
                return err.into_response();
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
