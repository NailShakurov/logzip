use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use tower_http::cors::{Any, CorsLayer};
use crate::tools;

const MAX_BODY_BYTES: usize = 512 * 1024; // 512 KB — предотвращает тяжёлую компрессию
const RATE_WINDOW: Duration = Duration::from_secs(60);
const RATE_MAX: u32 = 20; // запросов в минуту с одного IP
const MAX_CONCURRENT: usize = 3;

#[derive(Clone)]
struct AppState {
    rate_limiter: Arc<Mutex<HashMap<IpAddr, (u32, Instant)>>>,
    semaphore: Arc<Semaphore>,
}

pub async fn serve() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let state = AppState {
        rate_limiter: Arc::new(Mutex::new(HashMap::new())),
        semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
    };

    let app = Router::new()
        .route("/mcp", post(handle_mcp))
        .route("/health", get(|| async { "ok" }))
        .route("/.well-known/mcp/server-card.json", get(server_card))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[logzip-mcp] HTTP listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    // Fly.io выставляет Fly-Client-IP для реального IP за прокси
    headers
        .get("fly-client-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
}

fn check_rate(state: &AppState, ip: IpAddr) -> bool {
    let mut map = state.rate_limiter.lock().unwrap();
    let now = Instant::now();
    // Чистим накопившиеся записи раз в 1000 IP
    if map.len() > 1000 {
        map.retain(|_, (_, t)| now.duration_since(*t) < RATE_WINDOW * 2);
    }
    let entry = map.entry(ip).or_insert((0, now));
    if now.duration_since(entry.1) >= RATE_WINDOW {
        *entry = (1, now);
        true
    } else if entry.0 < RATE_MAX {
        entry.0 += 1;
        true
    } else {
        false
    }
}

async fn handle_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let id = body.get("id").cloned().unwrap_or(Value::Null);

    // Rate limit по IP
    if let Some(ip) = client_ip(&headers) {
        if !check_rate(&state, ip) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32000, "message": "Rate limit exceeded: 20 req/min per IP" }
                })),
            );
        }
    }

    // Ограничение параллельных запросов
    let _permit = match state.semaphore.try_acquire() {
        Ok(p) => p,
        Err(_) => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32000, "message": "Server busy, retry in a moment" }
            })),
        ),
    };

    // Notifications (без "id") — тихо игнорируем
    if !body.as_object().map(|o| o.contains_key("id")).unwrap_or(false) {
        return (StatusCode::OK, Json(json!({})));
    }

    let method = match body["method"].as_str() {
        Some(m) => m,
        None => return (StatusCode::OK, Json(err(id, -32600, "Missing method"))),
    };
    let params = body.get("params");

    let result = dispatch(method, params);
    let resp = match result {
        Ok(r)  => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
    };
    (StatusCode::OK, Json(resp))
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

async fn server_card() -> Json<Value> {
    Json(json!({
        "name": "logzip",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Compress logs for LLM analysis — native Rust MCP server",
        "tools": [{
            "name": "compress_content",
            "description": "Compress log text pasted directly into the conversation using logzip.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Log text to compress" },
                    "quality": { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" }
                },
                "required": ["content"]
            }
        }]
    }))
}

fn err(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
