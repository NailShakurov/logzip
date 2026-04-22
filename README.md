# logzip (Rust)

[![PyPI version](https://img.shields.io/pypi/v/logzip.svg)](https://pypi.org/project/logzip/)
[![PyPI downloads](https://img.shields.io/pypi/dm/logzip.svg)](https://pypi.org/project/logzip/)
[![Python 3.9+](https://img.shields.io/pypi/pyversions/logzip.svg)](https://pypi.org/project/logzip/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/powered%20by-Rust-orange.svg)](https://www.rust-lang.org/)

Compress logs **before** sending to LLM. Powered by Rust & PyO3.

```text
raw log → [logzip compress] → compressed text → LLM (Claude Code / Cursor / API)
```

### Before / After

**Raw Log (Uvicorn):**
```text
INFO: 127.0.0.1:45678 - "GET /api/v1/status HTTP/1.1" 200 OK
INFO: 127.0.0.1:45679 - "GET /api/v1/status HTTP/1.1" 200 OK
... (100 similar lines) ...
```

**logzip output:**
```text
--- PREFIX ---
INFO: 127.0.0.1:
--- LEGEND ---
#0# = - "GET /api/v1/status HTTP/1.1" 200 OK
--- BODY ---
45678 #0#
45679 #0#
...
```

Typical savings: **52–58%** on structured logs (systemd, uvicorn, docker).  
Anomalies and unique lines stay uncompressed — visible at a glance in the BODY.

### Why use logzip? (RAG & LLM)

When working with logs in LLMs (Claude, GPT, RAG systems), you face two problems:
1. **Context Limit**: Logs are huge. A 10MB log is ~2.5M tokens.
2. **Noise**: 90% of the log consists of repeating `INFO` and identical requests that drown out the real error.

`logzip` is well-suited for **RAG pipelines**: it compresses the context before sending it to the model, saving money on tokens and increasing answer accuracy by highlighting anomalies.

---

## Performance (7.96 MB Log, ~2M tokens)

Benchmarked on a real 7.96 MB production log.

### logzip modes

| Mode | CLI | Time (ms) | Size (KB) | Saved (%) | Output type |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **fast** | `--quality fast` | ~200 | ~4,900 | ~40% | text/LLM |
| **balanced** | `--quality balanced` | 404 | 3,928 | 52% | text/LLM |
| **recursive** ★ | `--quality balanced --bpe-passes 2` | 418 | 3,404 | **58%** | text/LLM |
| **max** | `--quality max` | 507 | 3,511 | 57% | text/LLM |

★ **recursive** (`balanced` + 2 BPE passes) beats `max` in both size and speed — recommended for production.

### vs. binary compressors (for context)

| Tool | Time (ms) | Size (KB) | Saved (%) | LLM-readable? |
| :--- | :--- | :--- | :--- | :--- |
| lz4 | 6 | 1,280 | 84% | No |
| zstd (lvl 3) | 14 | 819 | 90% | No |
| zlib (lvl 6) | 69 | 840 | 90% | No |
| **logzip (recursive)** | 418 | 3,404 | 58% | **Yes** |

Binary compressors produce opaque binary blobs — LLMs cannot read them. logzip trades ~30% size for fully human- and LLM-readable output.

Token estimation: 1 token ≈ 4 characters (rough estimate for English-like logs).

### Economic Impact

```text
┌──────────────────────────────────────────────────────────┐
│  logzip Savings (7.96 MB Production Log)                 │
├──────────────────────────────────────────────────────────┤
│  Raw Size:        8,151 KB  (~1,990,000 tokens)          │
│  After balanced:  3,928 KB  (~959,000 tokens,  -52%)     │
│  After recursive: 3,404 KB  (~831,000 tokens,  -58%)     │
├──────────────────────────────────────────────────────────┤
│  Cost Before:     $5.97                                  │
│  Cost After:      $2.49      (Claude 3.5 Sonnet Input)   │
│  LLM Efficiency:  2.4x larger context for the same price │
└──────────────────────────────────────────────────────────┘
```

---

## Install

```bash
pip install logzip
```

## CLI

```bash
# stdin → stdout (default mode)
logzip compress < app.log

# quality preset (fast|balanced|max)
logzip compress --quality balanced < app.log

# explicit BPE passes (overrides --quality default)
logzip compress --quality balanced --bpe-passes 3 < app.log

# with preamble (LLM decode instructions at the top)
logzip compress --preamble < app.log > compressed.txt

# save + show stats
logzip compress --stats -i app.log -o app.logzip

# explicit profile (otherwise auto-detected)
logzip compress --profile journalctl < /tmp/syslog.txt

# decompress
logzip decompress -i app.logzip
```

## Python API

```python
from logzip import compress, decompress

# compress
result = compress(raw_log_text)
print(result.render(with_preamble=True))   # → for LLM
print(result.stats_str())                  # → for logs

# fine-grained control
result = compress(
    raw_log_text,
    max_legend_entries=128,   # legend size
    bpe_passes=2,             # recursive BPE passes (1–3)
    do_normalize=True,        # collapse timestamps, ANSI, IPs
    do_templates=True,        # structural template extraction
)

# decompress
original = decompress(result.render())
```

## Through the eyes of an LLM

Unlike `gzip/zstd` which produce binary noise, `logzip` produces **structured text**. The model can reliably interpret the legend and reconstruct repeated patterns, allowing it to analyze the log directly in compressed form.

**Input for LLM:**
> This is a compressed log. Rules: `#0#` is replaced by `GET /api/v1/status`.
>
> --- BODY ---
> 12:00:01 #0# 200 OK
> 12:00:02 #0# 500 ERR <-- Boom, anomaly!

The model instantly spots the 500 error without wading through thousands of identical successful requests.

## Architecture & Safety

1. **Normalizer**: Collapses ANSI, timestamps, IPs, and common prefixes.
2. **Frequency Analysis**: Parallel n-gram counting using `rayon`.
3. **Greedy Legend**: Optimized selection using a positional index (O(N)).
4. **Direct Replacement**: Fast substitution without re-scanning.
5. **Recursive BPE**: Second-pass compression on already-compressed text — finds repeated tag sequences for extra savings.
6. **Templates**: Structural template extraction.

### Safety First
- **Pure Rust**: Core logic is 100% Rust.
- **Zero `unsafe`**: The codebase contains **no unsafe blocks**, ensuring memory safety within the Python runtime.
- **Stress-tested**: Handled multi-GB logs without memory leaks or crashes.

## Reproducibility

Want to verify our benchmarks? Run the included script:
```bash
python benchmark.py
```

## Roadmap / v2

- [ ] MCP server for Claude Code
- [ ] Suffix automaton for arbitrary repetition search
- [ ] Streaming mode for massive files
