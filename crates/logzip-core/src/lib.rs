pub mod base62;
pub mod compress;
pub mod legend;
pub mod normalizer;
pub mod profiles;
pub mod templates;

pub use compress::{compress, compress_best, decompress, CompressResult, PreserveConfig, PREAMBLE};
