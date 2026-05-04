use axum::{
    http::Method,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};
use crate::tools;

pub async fn serve() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/mcp", post(handle_mcp))
        .route("/health", get(|| async { "ok" }))
        .layer(cors);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[logzip-mcp] HTTP listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_mcp(Json(body): Json<Value>) -> impl IntoResponse {
    // Notifications (no "id" key) — acknowledge silently
    if !body.as_object().map(|o| o.contains_key("id")).unwrap_or(false) {
        return Json(json!({}));
    }

    let id = body["id"].clone();
    let method = match body["method"].as_str() {
        Some(m) => m,
        None => return Json(err(id, -32600, "Missing method")),
    };
    let params = body.get("params");

    let result = dispatch(method, params);
    Json(match result {
        Ok(r)  => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
    })
}

fn dispatch(method: &str, params: Option<&Value>) -> Result<Value, tools::RpcError> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "logzip", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => tools::list_http(),
        "tools/call" => {
            let p = params.ok_or_else(|| tools::RpcError { code: -32602, message: "Missing params".into() })?;
            let name = p["name"].as_str()
                .ok_or_else(|| tools::RpcError { code: -32602, message: "Missing tool name".into() })?;
            match name {
                "compress_content" => tools::compress_content(&p["arguments"]),
                _ => Err(tools::RpcError {
                    code: -32602,
                    message: format!("Tool '{}' requires local installation. See https://github.com/NailShakurov/logzip", name),
                }),
            }
        }
        _ => Err(tools::RpcError { code: -32601, message: format!("Method not found: {}", method) }),
    }
}

fn err(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
