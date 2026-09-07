#[cfg(feature = "capture")]
pub mod output;

pub mod check;
pub mod checker;
pub mod completion;
pub mod doctests;
pub mod error;
pub mod ffi;
pub mod format;
pub mod host;
pub mod input;
pub mod lints;
pub mod panic;
pub mod repl;
pub mod run;
pub mod shell;
pub mod spython_lib;

pub use check::{
    TypeErrors, Warnings, annotation_check, check_file_doctests, check_source,
    collect_import_files, make_project_metadata, modules_beside, print_type_errors,
};
pub(crate) use check::{infer_expression_type, type_check_repl_input};
pub use checker::Level;
pub use error::{SpythonError, read_file, show_error};
pub use format::format_source;
pub use repl::{Failed, REPL_ERROR, REPL_OK, REPL_QUIT, Repl, ReplOutcome, repl_new, repl_run};
pub use run::{DOCTEST_RUNNER, new_interpreter, run_source};

pub use ty_project::ProjectDatabase;

/// Package version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Libraries bundled in this build (set by build.rs).
pub const LIBS_VERSION: &str = env!("LIBS_VERSION");
/// Build date in YYYY-MM-DD format (set by build.rs).
pub const BUILD_DATE: &str = env!("BUILD_DATE");
/// Short git commit hash (set by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");

/// The version line the binary prints: `spython 0.3.0 (rustpython ..., 2026-09-07, abc1234)`.
pub fn version() -> String {
    format!("spython {}", version_short())
}

/// The same without the name, for a caller that already said whose version it is.
pub fn version_short() -> String {
    format!("{VERSION} ({LIBS_VERSION}, {BUILD_DATE}, {GIT_HASH})")
}
