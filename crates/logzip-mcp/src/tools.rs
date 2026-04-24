use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use crate::sandbox::Sandbox;

#[derive(serde::Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

pub fn list() -> Result<Value, RpcError> {
    Ok(json!({
        "tools": [
            {
                "name": "compress_file",
                "description": "Compress an entire log file using logzip and return the result ready for LLM analysis.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path":             { "type": "string", "description": "Absolute path to the log file" },
                        "quality":          { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" },
                        "preserve_patterns": {
                            "type": "array", "items": { "type": "string" },
                            "description": "Extra regex patterns to keep in body (e.g. REQ-\\d+-\\w+). Use strict anchors ^ and $."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "compress_tail",
                "description": "Compress only the last N lines of a log file. Efficient for large files — does not load the entire file into memory.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path":             { "type": "string", "description": "Absolute path to the log file" },
                        "lines":            { "type": "integer", "minimum": 1, "default": 500, "description": "Number of tail lines to compress" },
                        "quality":          { "type": "string", "enum": ["fast", "balanced", "max"], "default": "balanced" },
                        "preserve_patterns": {
                            "type": "array", "items": { "type": "string" },
                            "description": "Extra regex patterns to keep in body (e.g. REQ-\\d+-\\w+). Use strict anchors ^ and $."
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "get_stats",
                "description": "Return file metadata and token estimates without compressing. Call this first to decide whether to use compress_file or compress_tail.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the log file" }
                    },
                    "required": ["path"]
                }
            }
        ]
    }))
}

pub fn call(params: Option<&Value>, sandbox: &Sandbox) -> Result<Value, RpcError> {
    let params = params.ok_or_else(|| RpcError { code: -32602, message: "Missing params".into() })?;
    let name = params["name"].as_str()
        .ok_or_else(|| RpcError { code: -32602, message: "Missing tool name".into() })?;
    let args = &params["arguments"];

    let path_str = args["path"].as_str()
        .ok_or_else(|| RpcError { code: -32602, message: "Missing required argument: path".into() })?;

    let path = sandbox.validate(path_str)
        .map_err(|e| RpcError { code: -32602, message: e })?;

    // preserve_ids always true for MCP — context accuracy over compression ratio
    let extra_patterns: Vec<String> = args["preserve_patterns"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let preserve = logzip_core::PreserveConfig { preserve_ids: true, extra_patterns };

    match name {
        "compress_file" => {
            let quality = args["quality"].as_str().unwrap_or("balanced");
            compress_file_impl(&path, quality, &preserve).map_err(|e| RpcError { code: -32603, message: e })
        }
        "compress_tail" => {
            let lines = args["lines"].as_u64().unwrap_or(500) as usize;
            let quality = args["quality"].as_str().unwrap_or("balanced");
            compress_tail_impl(&path, lines, quality, &preserve).map_err(|e| RpcError { code: -32603, message: e })
        }
        "get_stats" => {
            get_stats_impl(&path).map_err(|e| RpcError { code: -32603, message: e })
        }
        _ => Err(RpcError { code: -32602, message: format!("Unknown tool: {}", name) })
    }
}

pub fn compress_tail_internal(path: &Path, lines: usize, quality: &str) -> Result<String, RpcError> {
    let text = read_tail(path, lines)
        .map_err(|e| RpcError { code: -32603, message: format!("Cannot read tail: {}", e) })?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let preserve = logzip_core::PreserveConfig { preserve_ids: true, extra_patterns: vec![] };
    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, bpe_passes, Some(&preserve));
    Ok(result.render(true))
}

// ─── Внутренние реализации ────────────────────────────────────────────────────

fn content_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

fn quality_params(quality: &str) -> (usize, usize) {
    match quality {
        "max"      => (512, 2),
        "balanced" => (128, 1),
        _          => (32,  1),
    }
}

fn compress_file_impl(path: &Path, quality: &str, preserve: &logzip_core::PreserveConfig) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {}", e))?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, bpe_passes, Some(preserve));
    log_preserved(&result);
    Ok(content_text(result.render(true)))
}

fn compress_tail_impl(path: &Path, lines: usize, quality: &str, preserve: &logzip_core::PreserveConfig) -> Result<Value, String> {
    let text = read_tail(path, lines)
        .map_err(|e| format!("Cannot read file tail: {}", e))?;
    let (max_legend, bpe_passes) = quality_params(quality);
    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, bpe_passes, Some(preserve));
    log_preserved(&result);
    Ok(content_text(result.render(true)))
}

fn log_preserved(result: &logzip_core::CompressResult) {
    let n: usize = result.stats.get("preserved_candidates")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if n > 0 {
        eprintln!("[logzip-mcp] {n} candidate(s) preserved from legend (IDs/patterns kept in body)");
    }
}

fn get_stats_impl(path: &Path) -> Result<Value, String> {
    let meta = std::fs::metadata(path)
        .map_err(|e| format!("Cannot stat file: {}", e))?;
    let file_size = meta.len();
    let estimated_tokens = file_size / 4;
    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Определяем профиль через mini-compress первых 4096 байт.
    // profiles::auto_detect может быть pub(crate) — безопаснее использовать compress().
    let sample_len = (4096usize).min(file_size as usize);
    let mut sample_buf = vec![0u8; sample_len];
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    f.read(&mut sample_buf).map_err(|e| e.to_string())?;
    let sample_str = String::from_utf8_lossy(&sample_buf);
    let mini = logzip_core::compress(&sample_str, 1, 1, false, None, false, 1, None);
    let detected_profile = mini.detected_profile;

    let recommended_tool = if estimated_tokens > 50_000 {
        "compress_tail"
    } else {
        "compress_file"
    };

    let stats = json!({
        "file_size_bytes":  file_size,
        "estimated_tokens": estimated_tokens,
        "detected_profile": detected_profile,
        "file_name":        file_name,
        "recommended_tool": recommended_tool
    });

    Ok(content_text(serde_json::to_string_pretty(&stats).unwrap()))
}

fn read_tail(path: &Path, n_lines: usize) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.seek(SeekFrom::End(0))?;
    if size == 0 {
        return Ok(String::new());
    }

    const CHUNK: u64 = 4096;
    let mut newlines = 0usize;
    let mut scan_pos = size;
    let mut found_pos = 0u64;

    'outer: loop {
        let chunk_end = scan_pos;
        let chunk_start = scan_pos.saturating_sub(CHUNK);
        let chunk_size = (chunk_end - chunk_start) as usize;

        file.seek(SeekFrom::Start(chunk_start))?;
        let mut buf = vec![0u8; chunk_size];
        file.read_exact(&mut buf)?;

        for i in (0..chunk_size).rev() {
            if buf[i] == b'\n' {
                newlines += 1;
                if newlines > n_lines {
                    found_pos = chunk_start + i as u64 + 1;
                    break 'outer;
                }
            }
        }

        if chunk_start == 0 {
            break;
        }
        scan_pos = chunk_start;
    }

    file.seek(SeekFrom::Start(found_pos))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;

    fn sample_log(n: usize) -> String {
        (0..n).map(|i| format!("2024-01-01T00:00:{:02}Z INFO request id={} status=200\n", i % 60, i))
              .collect()
    }

    fn default_preserve() -> logzip_core::PreserveConfig {
        logzip_core::PreserveConfig { preserve_ids: false, extra_patterns: vec![] }
    }

    #[test]
    fn test_compress_file_returns_content_array() {
        let tmp = env::temp_dir().join("logzip_tools_test_cf.log");
        fs::write(&tmp, sample_log(50)).unwrap();
        let result = compress_file_impl(&tmp, "fast", &default_preserve()).unwrap();
        assert!(result["content"].is_array());
        assert_eq!(result["content"][0]["type"], "text");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_compress_tail_returns_last_n_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_ct.log");
        fs::write(&tmp, sample_log(200)).unwrap();
        let result = compress_tail_impl(&tmp, 10, "fast", &default_preserve()).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("BODY"));
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_get_stats_fields() {
        let tmp = env::temp_dir().join("logzip_tools_test_gs.log");
        fs::write(&tmp, sample_log(100)).unwrap();
        let result = get_stats_impl(&tmp).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let stats: Value = serde_json::from_str(text).unwrap();
        assert!(stats["file_size_bytes"].as_u64().unwrap() > 0);
        assert!(stats["estimated_tokens"].as_u64().unwrap() > 0);
        assert!(stats["recommended_tool"].as_str().is_some());
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_preserve_ids_ip_stays_in_body() {
        let log: String = (0..100)
            .map(|i| format!("2024-01-01 INFO request from 192.168.1.100 path=/api/{i}\n"))
            .collect();
        let tmp = env::temp_dir().join("logzip_preserve_ip_test.log");
        fs::write(&tmp, &log).unwrap();
        let preserve = logzip_core::PreserveConfig { preserve_ids: true, extra_patterns: vec![] };
        let result = compress_file_impl(&tmp, "balanced", &preserve).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // IP must not appear as a legend value (would be "= 192.168.1.100")
        assert!(!text.contains("= 192.168.1.100"), "IP should not be a legend entry");
        // IP must be visible in body
        assert!(text.contains("192.168.1.100"), "IP should remain visible in body");
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_preserve_custom_pattern() {
        let log: String = (0..50)
            .map(|i| format!("2024-01-01 INFO trace REQ-{i:05}-XYZ status=200\n"))
            .collect();
        let tmp = env::temp_dir().join("logzip_preserve_custom_test.log");
        fs::write(&tmp, &log).unwrap();
        let preserve = logzip_core::PreserveConfig {
            preserve_ids: false,
            extra_patterns: vec![r"^REQ-\d+-XYZ$".to_string()],
        };
        let result = compress_file_impl(&tmp, "balanced", &preserve).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains("= REQ-"), "custom pattern should not be a legend entry");
        fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn test_read_tail_exact_lines() {
        let tmp = env::temp_dir().join("logzip_tools_test_rt.log");
        let lines: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
        fs::write(&tmp, lines.join("\n") + "\n").unwrap();
        let tail = read_tail(&tmp, 10).unwrap();
        let tail_lines: Vec<&str> = tail.trim_end().split('\n').collect();
        assert_eq!(tail_lines.len(), 10);
        assert_eq!(tail_lines[0], "line 90");
        assert_eq!(tail_lines[9], "line 99");
        fs::remove_file(tmp).unwrap();
    }
}
