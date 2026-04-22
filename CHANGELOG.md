# Changelog

All notable changes to this project will be documented in this file.

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
