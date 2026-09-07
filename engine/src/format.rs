//! Formatting Python source, with ruff's formatter.

use ruff_python_formatter::{DocstringCode, PyFormatOptions, QuoteStyle, format_module_source};

/// What only the command line asks for: formatting the files it names, which
/// the page has not got.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::run;

/// Format Python source with ruff's formatter.
/// Returns `Err(message)` if the source cannot be parsed.
pub fn format_source(source: &str) -> Result<String, String> {
    let options = PyFormatOptions::default()
        .with_docstring_code(DocstringCode::Enabled)
        .with_quote_style(QuoteStyle::Single);
    format_module_source(source, options)
        .map(|f| f.into_code())
        .map_err(|e| e.to_string())
}
