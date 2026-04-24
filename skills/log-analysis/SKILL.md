---
name: log-analysis
description: Use this skill when the user asks to analyze, investigate, debug, or review a log file — or when given a path to a .log, syslog, journalctl output, or similar file. Prescribes using logzip MCP tools instead of reading the file directly.
---

# Log Analysis with logzip

When the user provides a log file path for analysis, ALWAYS use logzip MCP tools instead of reading the file directly with Read/Bash/grep.

## Why

Direct file reading loads the raw log into context — often hundreds of thousands of tokens for a multi-MB file. logzip compresses logs 50–60% while keeping the output fully readable by LLMs. Anomalies and unique lines remain visible; repetitive noise is collapsed into a legend.

## Workflow

**Step 1 — get_stats first:**
```
mcp__logzip__get_stats(path)
```
Returns `file_size_bytes`, `estimated_tokens`, `detected_profile`, and `recommended_tool`.

**Step 2 — compress based on recommendation:**

If `recommended_tool == "compress_file"` (file < 200K tokens):
```
mcp__logzip__compress_file(path, quality="balanced")
```

If `recommended_tool == "compress_tail"` (large file):
```
mcp__logzip__compress_tail(path, lines=500, quality="balanced")
```
For more context on large files, increase `lines` (e.g. 2000).

**Step 3 — analyze the compressed output:**

The output has three sections:
- `PREFIX` — common prefix stripped from all lines (read once, ignore for anomaly search)
- `LEGEND` — substitution dictionary: `#0#` → actual repeated string
- `BODY` — compressed log body; lines with NO substitutions are unique — these are the anomalies

Focus on lines in BODY that contain no `#N#` tokens — those are the interesting events.

## Tool names

Depending on Claude Code version, tools may appear as:
- `mcp__logzip__get_stats`
- `mcp__logzip__compress_file`
- `mcp__logzip__compress_tail`

## Prerequisite

This skill requires the logzip MCP server to be registered. If tools are not available, tell the user:
> "logzip MCP is not configured. Install it with: `claude mcp add logzip -- logzip mcp --allow-dir /var/log`"
