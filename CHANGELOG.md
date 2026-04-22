# Changelog

All notable changes to this project will be documented in this file.

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
