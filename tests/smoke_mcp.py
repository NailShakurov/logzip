#!/usr/bin/env python3
"""Smoke test for logzip MCP server — sends JSON-RPC via stdin."""
import subprocess
import json
import sys
import os
import tempfile

BINARY = os.path.join(os.path.dirname(__file__), "..", "target", "release", "logzip")


def send_recv(proc, msg):
    line = json.dumps(msg) + "\n"
    proc.stdin.write(line.encode())
    proc.stdin.flush()
    response = proc.stdout.readline()
    assert response, "Server returned empty response"
    return json.loads(response)


def send_notification(proc, msg):
    line = json.dumps(msg) + "\n"
    proc.stdin.write(line.encode())
    proc.stdin.flush()


def main():
    with tempfile.NamedTemporaryFile(mode='w', suffix='.log', dir='/tmp', delete=False) as f:
        for i in range(200):
            f.write(f"2024-01-01T00:{i//60:02}:{i%60:02}Z INFO request id={i} path=/api/v1/users status=200\n")
        tmp_path = f.name

    proc = subprocess.Popen(
        [BINARY, "mcp", "--allow-dir", "/tmp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    try:
        # 1. initialize
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "0.1"}}
        })
        assert "result" in resp, f"initialize failed: {resp}"
        assert resp["result"]["serverInfo"]["name"] == "logzip"
        print("✓ initialize")

        # 2. notification — сервер не должен отвечать (проверяем через следующий ping)
        send_notification(proc, {"jsonrpc": "2.0", "method": "notifications/initialized"})

        # 3. ping
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 2, "method": "ping"})
        assert resp.get("result") == {}, f"ping failed: {resp}"
        print("✓ ping (notification had no response)")

        # 4. tools/list
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 3, "method": "tools/list"})
        tool_names = {t["name"] for t in resp["result"]["tools"]}
        assert tool_names == {"compress_file", "compress_tail", "get_stats"}, f"Wrong tools: {tool_names}"
        print("✓ tools/list")

        # 5. get_stats
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "get_stats", "arguments": {"path": tmp_path}}
        })
        stats = json.loads(resp["result"]["content"][0]["text"])
        assert stats["file_size_bytes"] > 0
        assert stats["recommended_tool"] in ("compress_file", "compress_tail")
        print("✓ get_stats")

        # 6. compress_file
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "compress_file", "arguments": {"path": tmp_path, "quality": "fast"}}
        })
        text = resp["result"]["content"][0]["text"]
        assert "BODY" in text, f"No BODY in compress output"
        print("✓ compress_file")

        # 7. compress_tail
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": {"name": "compress_tail", "arguments": {"path": tmp_path, "lines": 20, "quality": "fast"}}
        })
        assert "BODY" in resp["result"]["content"][0]["text"]
        print("✓ compress_tail")

        # 8. sandbox violation
        resp = send_recv(proc, {
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": {"name": "get_stats", "arguments": {"path": "/etc/passwd"}}
        })
        assert "error" in resp, f"Sandbox violation should return error: {resp}"
        assert resp["error"]["code"] == -32602
        print("✓ sandbox violation rejected")

        # 9. unknown method → MethodNotFound
        resp = send_recv(proc, {"jsonrpc": "2.0", "id": 8, "method": "nonexistent/method"})
        assert resp.get("error", {}).get("code") == -32601
        print("✓ MethodNotFound")

    finally:
        proc.terminate()
        os.unlink(tmp_path)

    print("\n✅ All MCP smoke tests passed!")


if __name__ == "__main__":
    main()
