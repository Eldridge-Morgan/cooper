use crate::js::JsRuntime;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Server-side rendering engine.
///
/// Renders pages by calling into the JS worker pool, which executes
/// the page component and returns HTML. Islands are serialized as
/// placeholder divs with data attributes for client-side hydration.
pub struct SsrRenderer;

impl SsrRenderer {
    /// Render a page to HTML.
    pub async fn render_page(
        runtime: &JsRuntime,
        source_file: &str,
        params: &HashMap<String, String>,
    ) -> Result<String> {
        let input = serde_json::json!({
            "source": source_file,
            "params": params,
        });

        let result = runtime
            .call_handler(source_file, "__cooper_ssr_render", &input)
            .await;

        match result {
            Ok(json_str) => {
                // Parse the result — expected: { "html": "...", "islands": [...], "head": "..." }
                let parsed: Value = serde_json::from_str(&json_str)
                    .unwrap_or_else(|_| serde_json::json!({ "html": json_str }));

                let html = parsed
                    .get("html")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let head = parsed
                    .get("head")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let islands: Vec<IslandData> = parsed
                    .get("islands")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let full_html = build_html_document(html, head, &islands);
                Ok(full_html)
            }
            Err(_) => {
                // Fallback: render a simple HTML page with error info
                Ok(build_html_document(
                    &format!(
                        r#"<div style="font-family:system-ui;padding:2rem;">
                        <h1>Page: {}</h1>
                        <p>SSR rendering requires the Cooper SDK bridge to implement page rendering.</p>
                        <p style="color:#666">Params: {:?}</p>
                    </div>"#,
                        source_file, params
                    ),
                    "",
                    &[],
                ))
            }
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct IslandData {
    id: String,
    component: String,
    props: Value,
    hydrate: String,
}

/// Build a full HTML document with the rendered page content,
/// island hydration markers, and the client-side hydration script.
fn build_html_document(body: &str, head: &str, islands: &[IslandData]) -> String {
    let island_script = if islands.is_empty() {
        String::new()
    } else {
        let island_data: Vec<Value> = islands
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.id,
                    "component": i.component,
                    "props": i.props,
                    "hydrate": i.hydrate,
                })
            })
            .collect();

        format!(
            r#"<script type="module">
// Cooper Island Hydration
const islands = {};

async function hydrateIsland(island) {{
  const el = document.getElementById(`cooper-island-${{island.id}}`);
  if (!el) return;

  const mod = await import(`/_cooper/islands/${{island.component}}.js`);
  const Component = mod.default;

  switch (island.hydrate) {{
    case 'load':
      hydrate(el, Component, island.props);
      break;
    case 'visible':
      new IntersectionObserver((entries, obs) => {{
        if (entries[0].isIntersecting) {{
          hydrate(el, Component, island.props);
          obs.disconnect();
        }}
      }}).observe(el);
      break;
    case 'idle':
      requestIdleCallback(() => hydrate(el, Component, island.props));
      break;
    case 'interaction':
      ['click', 'focus', 'mouseover'].forEach(evt =>
        el.addEventListener(evt, () => hydrate(el, Component, island.props), {{ once: true }})
      );
      break;
    case 'none':
      break;
  }}
}}

function hydrate(el, Component, props) {{
  // React hydration
  if (window.__COOPER_REACT__) {{
    window.__COOPER_REACT__.hydrateRoot(el, window.__COOPER_REACT__.createElement(Component, props));
  }}
}}

islands.forEach(hydrateIsland);
</script>"#,
            serde_json::to_string(&island_data).unwrap_or_default()
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  {head}
  <style>
    *, *::before, *::after {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: system-ui, -apple-system, sans-serif; }}
  </style>
</head>
<body>
  <div id="cooper-app">{body}</div>
  {island_script}
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_html_document_basic() {
        let html = build_html_document("<h1>Hello</h1>", "<title>Test</title>", &[]);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<title>Test</title>"));
        assert!(!html.contains("cooper-island")); // no islands
    }

    #[test]
    fn test_build_html_document_with_islands() {
        let islands = vec![IslandData {
            id: "btn-1".to_string(),
            component: "LikeButton".to_string(),
            props: serde_json::json!({"count": 42}),
            hydrate: "load".to_string(),
        }];
        let html = build_html_document("<div>Page</div>", "", &islands);
        assert!(html.contains("cooper-island"));
        assert!(html.contains("LikeButton"));
    }
}
