mod bitmap;
pub mod check;
pub mod checker;
pub mod completion;
#[cfg(not(target_arch = "wasm32"))]
mod display;
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
mod scene;
pub mod shell;
pub mod spython_lib;

// What the frontends reach for. Everything else a module holds is reachable
// by its own path, such as `engine::check::annotation_check`.
pub use check::{check_source, print_type_errors};
pub(crate) use check::{infer_expression_type, type_check_repl_input};
pub use checker::Level;
pub use error::{SpythonError, color_stderr, color_stdout, read_file, show_error};
pub use format::format_source;
pub use repl::{Failed, Repl};
pub use run::{run_source, run_test_source};
pub use shell::{Shell, Status};

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
