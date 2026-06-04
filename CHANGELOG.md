# Changelog

All notable changes to this project will be documented in this file.

## [2.1.3] - 2026-06-04

### Fixed
- **Numeric precision loss on roundtrip**: the normalizer truncated ANY fraction with 6+ digits (`"time":0.000031435` → `0.000`, epoch microseconds). The `nanos_re` regex now matches timestamps only (`hh:mm:ss.fff…`) — plain numbers and latency fields are left untouched.
- Claude Code plugin: added `.claude-plugin/marketplace.json` — `claude plugin marketplace add` works as documented in README.
- README: corrected plugin install command (`logzip@logzip` — the marketplace name comes from marketplace.json, not the repo path).

### Added
- `--exact-timestamps` flag (Rust + Python CLI) and `exact_timestamps=False` kwarg (Python API): keeps full sub-second timestamp precision — lossless roundtrip.
- 4 Rust tests (normalizer) + 2 pytest (precision roundtrip).

### Changed
- Version synced across all spots (Cargo.toml, pyproject.toml, `__init__.py`, lib.rs); `llms.txt` rewritten for logzip (previously contained documentation for an unrelated project).
- README: removed stale "Zero Install (Cloud)" MCP section — cloud hosting was dropped in 2.1.2 era (`f5e1650`), local MCP only.

---

## [2.1.2] - 2026-04-24

### Added
- `PreserveConfig { preserve_ids, extra_patterns }` in logzip-core: keep identifiers (IP addresses and custom regexes) untouched during compression.
- MCP server: `preserve_ids=true` by default, `preserve_patterns` parameter in tool schemas.
- CLI: `--preserve-ids`, `--preserve-pattern <regex>` (repeatable), `--debug` flags.
- Python API: `preserve_ids=False`, `preserve_patterns=None` kwargs.
- 2 new tests: IP stays in body, custom pattern `REQ-\d+-XYZ`.
- `readme.workspace = true` for logzip-core and logzip-mcp crates (crates.io publishing).

---

## [2.1.1] - 2026-04-24

### Changed
- `[workspace.dependencies]` for logzip-core — removes `path+version` duplication in dependent crates.

### CI
- `publish.yml`: skip "already exists" error when publishing to crates.io (idempotency).

---

## [2.1.0] - 2026-04-24

### Added
- **Cargo workspace**: project split into three crates — `logzip-core` (algorithm), `logzip-py` (PyO3 bindings), `logzip-mcp` (MCP server).
- **MCP server** (`logzip-mcp`) — JSON-RPC 2.0 server for Claude Code and other MCP clients:
  - `compress_file` — compress a file by path.
  - `compress_tail` — compress the last N lines of a file.
  - `get_stats` — statistics without writing output.
  - `sandbox.rs` — path validation via `canonicalize` (path traversal protection).
- Single `logzip` binary with `compress`, `decompress`, `mcp` subcommands.
- **Claude Code plugin** (`.claude-plugin/plugin.json`) + `skills/log-analysis/SKILL.md` skill — auto-triggers MCP tools during log analysis.
- `smoke_mcp.py` — 9 integration scenarios for the MCP server.

### CI
- crates.io publish job in `publish.yml`.

---

## [1.1.0] - 2026-04-22

### Added
- Recursive BPE (meta-pass): second legend-selection pass on already-compressed text for 5–10% additional savings.
- `bpe_passes` parameter in Python API (`compress(..., bpe_passes=2)`).
- `--bpe-passes N` CLI flag — overrides the default set by `--quality`.
- `flatten_legend` decompressor: DFS + memoization replaces naive reverse substitution, correctly handles multi-pass DAG legends.
- Cyclic legend detection: malformed legends raise `ValueError` instead of hanging.
- `bpe_passes_used` stat in `result.stats()`.
- 7 new tests (15 total): tag collision, round-trip BPE×2, cyclic detection, CLI quality max.

### Changed
- `--quality max` now runs 2 BPE passes (512 legend entries).
- `--quality balanced` auto-upgrades to 2 passes for files > 5 MB.
- Recommended mode: `--quality balanced --bpe-passes 2` — beats `--quality max` in both size and speed on real logs.
- Version synced across `Cargo.toml`, `pyproject.toml`, `__init__.py`, `lib.rs`.

### Performance (7.96 MB production log)
- `--quality balanced`: 3928 KB, 404 ms, −52%
- `--quality balanced --bpe-passes 2`: 3404 KB, 418 ms, **−58%**
- `--quality max`: 3511 KB, 507 ms, −57%

## [0.1.0] - 2026-04-22

### Added
- Complete rewrite of the core compression engine in Rust (using PyO3).
- O(N log N) greedy legend selection algorithm with positional indexing.
- New CLI parameter `--quality` with presets: `fast`, `balanced`, `max`.
- Parallelized frequency analysis and indexing using `rayon`.
- Dynamic template extraction for further size reduction.
- Performance: ~200x speedup compared to the original Python implementation.
- Support for multiple normalization profiles (journalctl, docker, etc.).

### Fixed
- Algorithmic O(N²) bottleneck in legend generation.
- Memory usage issues with large log files.
- Threading model for concurrent compression tasks.
