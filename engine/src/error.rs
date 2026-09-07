use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use crate::{TypeErrors, print_type_errors};

/// What stops a spython command. Each one ends the run with a failing status,
/// after [`show_error`] has said what it was.
pub enum SpythonError {
    /// A file the user named that could not be read, either because it is not
    /// there or because the filesystem turned the read down.
    FileRead { path: PathBuf, err: std::io::Error },

    /// A file the user named that could not be written.
    FileWrite { path: PathBuf, err: std::io::Error },

    /// A path that names something other than a file, such as a directory.
    PathIsNotAFile { path: PathBuf },

    /// A path the checker cannot take. The project it builds addresses its
    /// files by name, and a name that is not UTF-8 has no place in it.
    NonUtf8Path { path: PathBuf },

    /// A failure of the ty project itself, before any file was checked.
    Database(String),

    /// The diagnostics of a file that does not check, with the database that
    /// prints them.
    TypeErrors(Box<TypeErrors>),

    /// A program that raised, and whose traceback the interpreter printed as
    /// it did.
    ProgramFailed,

    /// Doctests that failed, and said so as they ran.
    TestsFailed,

    /// A file the formatter turned down, with what it said about it.
    Format { path: PathBuf, message: String },

    /// The files `format --check` found unformatted.
    NotFormatted { paths: Vec<PathBuf> },

    /// Files the formatter turned down, which it named one by one as the run
    /// came to them.
    FormatFailed,

    /// A failure from outside the language, such as starting the repl reader.
    /// Shown as it came, as it already names its own subject.
    Other(Box<dyn std::error::Error>),
}

/// Say on stderr what stopped the run.
pub fn show_error(err: &SpythonError) {
    match err {
        SpythonError::FileRead { path, err } => {
            eprintln!("spython: cannot read '{}': {err}", path.display());
        }
        SpythonError::FileWrite { path, err } => {
            eprintln!("spython: cannot write '{}': {err}", path.display());
        }
        SpythonError::PathIsNotAFile { path } => {
            eprintln!("spython: '{}' is not a file", path.display());
        }
        SpythonError::NonUtf8Path { path } => {
            eprintln!(
                "spython: '{}' contains non-Unicode characters",
                path.display()
            );
        }
        SpythonError::Database(message) => eprintln!("spython: {message}"),
        SpythonError::Format { path, message } => {
            eprintln!("spython: cannot format '{}': {message}", path.display());
        }
        SpythonError::NotFormatted { paths } => {
            for path in paths {
                eprintln!("spython: '{}' is not formatted", path.display());
            }
        }
        SpythonError::TypeErrors(errors) => print_type_errors(
            &errors.db,
            &errors.diagnostics,
            std::io::stderr().is_terminal(),
        ),
        SpythonError::Other(err) => eprintln!("spython: {err}"),
        // Whoever failed already said so: the interpreter printed the
        // traceback, the doctest runner printed its own count, and the
        // formatter named each file it turned down.
        SpythonError::ProgramFailed | SpythonError::TestsFailed | SpythonError::FormatFailed => (),
    }
}

/// Read a file, naming it in whatever the filesystem says about the read.
pub fn read_file(path: &Path) -> Result<String, SpythonError> {
    std::fs::read_to_string(path).map_err(|err| SpythonError::FileRead {
        path: path.to_path_buf(),
        err,
    })
}
