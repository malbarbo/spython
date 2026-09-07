#[cfg(feature = "capture")]
pub mod output;

pub mod check;
pub mod checker;
pub mod completion;
pub mod doctests;
pub mod error;
pub mod ffi;
pub mod lints;
pub mod panic;
pub mod parser;
pub mod repl;
pub mod run;
pub mod spython_lib;
pub mod wasm_ffi;

pub use check::{
    TypeErrors, Warnings, annotation_check, check_file_doctests, check_source,
    collect_import_files, make_project_metadata, modules_beside, print_type_errors,
};
pub(crate) use check::{infer_expression_type, type_check_repl_input};
pub use checker::Level;
pub use error::{SpythonError, read_file, show_error};
pub use repl::{
    REPL_ERROR, REPL_OK, REPL_QUIT, ReplOutcome, ReplSession, ReplState, repl_new, repl_run,
};
pub use run::{DOCTEST_RUNNER, new_interpreter, run_source};

use ruff_python_formatter::{DocstringCode, PyFormatOptions, QuoteStyle, format_module_source};
pub use ty_project::ProjectDatabase;

/// Package version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Libraries bundled in this build (set by build.rs).
pub const LIBS_VERSION: &str = env!("LIBS_VERSION");
/// Build date in YYYY-MM-DD format (set by build.rs).
pub const BUILD_DATE: &str = env!("BUILD_DATE");
/// Short git commit hash (set by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");
/// Full version string: "0.1.0 (rustpython ..., 2026-04-01, abc1234)".
pub const LONG_VERSION: &str = env!("LONG_VERSION");

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
