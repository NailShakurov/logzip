use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

mod base62;
mod compress;
mod legend;
mod normalizer;
mod profiles;
mod templates;

use compress::compress as core_compress;
use compress::decompress as core_decompress;
use compress::CompressResult;

pub const PREAMBLE: &str = "\
# logzip/v1 — compressed log. Decode rules:
# #tag#  → replace with value from LEGEND
# &tag:v → replace with LEGEND &tag pattern, substitute @ with v
# PREFIX → prepend to every BODY line (if present)
";

/// Python-exposed compression result.
#[pyclass(name = "CompressResult")]
#[derive(Clone)]
pub struct PyCompressResult {
    #[pyo3(get)]
    body: String,
    #[pyo3(get)]
    legend: Vec<(String, String)>,
    #[pyo3(get)]
    templates: Vec<(String, String)>,
    #[pyo3(get)]
    common_prefix: String,
    #[pyo3(get)]
    detected_profile: String,
    stats_raw: HashMap<String, String>,
}

#[pymethods]
impl PyCompressResult {
    /// Render the compressed log to a string ready to send to LLM.
    #[pyo3(signature = (with_preamble = false))]
    fn render(&self, with_preamble: bool) -> String {
        let mut parts: Vec<String> = Vec::new();

        if with_preamble {
            parts.push(PREAMBLE.to_string());
        }

        if !self.common_prefix.is_empty() {
            parts.push(format!("--- PREFIX ---\n{}", self.common_prefix));
        }

        if !self.legend.is_empty() || !self.templates.is_empty() {
            parts.push("--- LEGEND ---".to_string());
            for (tag, value) in &self.legend {
                parts.push(format!("#{tag}# = {value}"));
            }
            for (tag, pattern) in &self.templates {
                parts.push(format!("&{tag} = {pattern}"));
            }
        }

        parts.push("--- BODY ---".to_string());
        parts.push(self.body.clone());

        parts.join("\n")
    }

    /// One-line stats summary.
    fn stats_str(&self) -> String {
        let s = &self.stats_raw;
        let profile = s.get("profile").map(|v| v.as_str()).unwrap_or("?");
        let orig = s.get("original_chars").map(|v| v.as_str()).unwrap_or("?");
        let comp = s.get("compressed_chars").map(|v| v.as_str()).unwrap_or("?");
        let ratio = s.get("ratio_pct").map(|v| v.as_str()).unwrap_or("?");
        let entries = s.get("legend_entries").map(|v| v.as_str()).unwrap_or("0");
        let tmpl = s.get("template_entries").map(|v| v.as_str()).unwrap_or("0");
        format!(
            "[logzip] profile={profile} | {orig} → {comp} chars ({ratio}% saved) | legend={entries} tmpl={tmpl}"
        )
    }

    /// Stats as Python dict.
    fn stats<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        let d = PyDict::new_bound(py);
        for (k, v) in &self.stats_raw {
            // Try to parse as int, then float, else string
            if let Ok(i) = v.parse::<i64>() {
                d.set_item(k, i).unwrap();
            } else if let Ok(f) = v.parse::<f64>() {
                d.set_item(k, f).unwrap();
            } else {
                d.set_item(k, v).unwrap();
            }
        }
        d
    }

    fn __repr__(&self) -> String {
        self.stats_str()
    }
}

impl From<CompressResult> for PyCompressResult {
    fn from(r: CompressResult) -> Self {
        Self {
            body: r.body,
            legend: r.legend.into_iter().map(|e| (e.tag, e.value)).collect(),
            templates: r
                .templates
                .into_iter()
                .map(|t| (t.tag, t.pattern))
                .collect(),
            common_prefix: r.common_prefix,
            detected_profile: r.detected_profile,
            stats_raw: r.stats,
        }
    }
}

// ─── Public API functions ─────────────────────────────────────────────────────

/// Compress log text. Returns CompressResult.
#[pyfunction]
#[pyo3(signature = (
    text,
    max_ngram = 2,
    max_legend_entries = 32,
    do_normalize = true,
    profile = None,
    do_templates = true,
    bpe_passes = 1,
))]
fn compress_log(
    text: String,
    max_ngram: usize,
    max_legend_entries: usize,
    do_normalize: bool,
    profile: Option<String>,
    do_templates: bool,
    bpe_passes: usize,
) -> PyResult<PyCompressResult> {
    let result = core_compress(
        &text,
        max_ngram,
        max_legend_entries,
        do_normalize,
        profile.as_deref(),
        do_templates,
        bpe_passes,
    );
    Ok(PyCompressResult::from(result))
}

/// Decompress rendered logzip output back to plain log.
#[pyfunction]
fn decompress_log(rendered: String) -> PyResult<String> {
    core_decompress(&rendered)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
}

/// Python module entry point.
#[pymodule]
fn _logzip(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compress_log, m)?)?;
    m.add_function(wrap_pyfunction!(decompress_log, m)?)?;
    m.add_class::<PyCompressResult>()?;
    m.add("__version__", "0.2.0")?;
    Ok(())
}

