# logzip (Rust)

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

Typical savings: **40–60%** on structured logs (systemd, uvicorn, docker).  
Anomalies and unique lines stay uncompressed — visible at a glance in the BODY.

### 🚀 Why use logzip? (RAG & LLM)

When working with logs in LLMs (Claude, GPT, RAG systems), you face two problems:
1. **Context Limit**: Logs are huge. A 10MB log is ~2.5M tokens.
2. **Noise**: 90% of the log consists of repeating `INFO` and identical requests that drown out the real error.

`logzip` is perfect for **RAG pipelines**: it compresses the context before sending it to the model, saving money on tokens and increasing answer accuracy by highlighting anomalies.

---

## Performance (8MB Log)

| Quality | Time (s) | Savings (%) | Tokens (est.) | Entries | Description |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **fast** | ~0.5s | 35-40% | ~1.2M | 32 | Default, near instant |
| **balanced** | ~0.4s | 50-55% | ~0.9M | 128 | Best for daily use |
| **max** | ~0.5s | 55-60% | ~0.8M | 512 | Max compression |

*Benchmarked on a real 8MB log (~2.0M tokens). Token estimation: 1 token ≈ 4 characters. Sub-second performance.*

---

## Install

```bash
pip install logzip
```

## CLI

```bash
# stdin → stdout (default mode)
cat app.log | logzip compress | pbcopy      # → buffer → paste to Claude

# with quality selection (fast|balanced|max)
logzip compress --quality balanced < app.log

# with preamble (LLM instructions at the beginning)
logzip compress --preamble < app.log > compressed.txt

# save + show stats
logzip compress --stats -i app.log -o app.logzip

# explicit profile (otherwise auto-detected)
logzip compress --profile journalctl < /tmp/syslog.txt
```

## Python API

```python
from logzip import compress, decompress

# compress
result = compress(raw_log_text, quality="balanced")
print(result.render(with_preamble=True))   # → for LLM
print(result.stats_str())                  # → for logs
```

## Through the eyes of an LLM

Unlike `gzip/zstd` which produce binary noise, `logzip` produces **structured text**. The model understands the legend and can "decompress" the log in its head or analyze it directly in compressed form.

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
5. **Templates**: Structural template extraction.

### Safety First
- **Pure Rust**: Core logic is 100% Rust.
- **Zero `unsafe`**: The codebase contains **no unsafe blocks**, ensuring memory safety within the Python runtime.
- **Audited**: No memory leaks or segment faults during multi-gigabyte log processing.

## Reproducibility

Want to verify our benchmarks? Run the included script:
```bash
python benchmark.py
```

## Roadmap / v2

- [ ] MCP server for Claude Code
- [ ] Suffix automaton for arbitrary repetition search
- [ ] Streaming mode for massive files
