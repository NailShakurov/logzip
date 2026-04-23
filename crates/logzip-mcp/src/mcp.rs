use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::BufRead;
use crate::sandbox::Sandbox;
use crate::tools;

#[derive(Deserialize)]
struct Request {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct Response {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<tools::RpcError>,
}

pub fn run(sandbox: Sandbox) {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut line = String::new();

    eprintln!("[logzip-mcp] ready, allowed dirs: {:?}", sandbox.allowed);

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => { eprintln!("[logzip-mcp] read error: {}", e); break; }
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[logzip-mcp] parse error: {}", e);
                let resp = Response {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(tools::RpcError { code: -32700, message: format!("Parse error: {}", e) }),
                };
                println!("{}", serde_json::to_string(&resp).unwrap());
                continue;
            }
        };

        // Notification (нет id) — обработать, не отвечать
        if request.id.is_none() {
            eprintln!("[logzip-mcp] notification: {}", request.method);
            continue;
        }

        let id = request.id.clone().unwrap();
        let result = handle_request(&request.method, request.params.as_ref(), &sandbox);

        let response = match result {
            Ok(r)  => Response { jsonrpc: "2.0", id, result: Some(r), error: None },
            Err(e) => Response { jsonrpc: "2.0", id, result: None,    error: Some(e) },
        };

        println!("{}", serde_json::to_string(&response).unwrap());
    }
}

fn handle_request(method: &str, params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, tools::RpcError> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {}, "prompts": {} },
            "serverInfo": { "name": "logzip", "version": env!("CARGO_PKG_VERSION") }
        })),

        "ping" => Ok(json!({})),

        "tools/list" => tools::list(),

        "tools/call" => tools::call(params, sandbox),

        "prompts/list" => Ok(json!({
            "prompts": [{
                "name": "analyze_logs",
                "description": "Compress and prepare a log file for SRE analysis",
                "arguments": [
                    { "name": "path",  "description": "Path to log file", "required": true },
                    { "name": "lines", "description": "Tail lines to compress (default: 500)", "required": false }
                ]
            }]
        })),

        "prompts/get" => {
            let params = params.ok_or_else(|| tools::RpcError { code: -32602, message: "Missing params".into() })?;
            if params["name"].as_str() != Some("analyze_logs") {
                return Err(tools::RpcError { code: -32602, message: "Unknown prompt".into() });
            }
            let path_str = params["arguments"]["path"].as_str()
                .ok_or_else(|| tools::RpcError { code: -32602, message: "Missing argument: path".into() })?;
            let lines = params["arguments"]["lines"].as_u64().unwrap_or(500) as usize;

            let path = sandbox.validate(path_str)
                .map_err(|e| tools::RpcError { code: -32602, message: e })?;

            let text = tools::compress_tail_internal(&path, lines, "balanced")?;

            Ok(json!({
                "messages": [{
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Ты — эксперт SRE. Перед тобой сжатый лог в формате logzip/v1.\n\
                             Сначала изучи секцию LEGEND — она содержит словарь замен.\n\
                             Затем читай BODY, ища аномалии, ошибки и паттерны.\n\n\
                             <compressed_log>\n{}\n</compressed_log>",
                            text
                        )
                    }
                }]
            }))
        }

        _ => Err(tools::RpcError { code: -32601, message: format!("Method not found: {}", method) }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_has_no_id() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn test_request_with_integer_id() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"tools/list"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!(42)));
    }

    #[test]
    fn test_request_with_string_id() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"ping"}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, Some(json!("abc-123")));
    }

    #[test]
    fn test_response_no_newlines_in_output() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: Some(json!({"tools": []})),
            error: None,
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains('\n'), "Response must be single-line: {}", s);
    }

    #[test]
    fn test_error_response_omits_result() {
        let resp = Response {
            jsonrpc: "2.0",
            id: json!(1),
            result: None,
            error: Some(tools::RpcError { code: -32601, message: "not found".into() }),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains("\"result\""), "result should be absent: {}", s);
        assert!(s.contains("\"error\""));
    }
}
