use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use crate::check::{TypeErrors, print_type_errors};

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

    /// What a format run found: the files the formatter turned down, with
    /// what it said about each, and the files `--check` found unformatted.
    Format {
        rejected: Vec<(PathBuf, String)>,
        unformatted: Vec<PathBuf>,
    },

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
        SpythonError::Format {
            rejected,
            unformatted,
        } => {
            for (path, message) in rejected {
                eprintln!("spython: cannot format '{}': {message}", path.display());
            }
            for path in unformatted {
                eprintln!("spython: '{}' is not formatted", path.display());
            }
        }
        SpythonError::TypeErrors(errors) => {
            print_type_errors(&errors.db, &errors.diagnostics, color_stderr())
        }
        SpythonError::Other(err) => eprintln!("spython: {err}"),
        // Whoever failed already said so: the interpreter printed the
        // traceback, and the doctest runner printed its own count.
        SpythonError::ProgramFailed | SpythonError::TestsFailed => (),
    }
}

/// Whether what goes to stderr takes colour. A terminal wants it, and a
/// redirected stderr keeps the codes out of the file it is being read into.
///
/// Every native decision about colour comes through here, so the answer is
/// the same wherever spython writes. The page decides for itself: it renders
/// the codes and asks for them by hand.
pub fn color_stderr() -> bool {
    std::io::stderr().is_terminal()
}

/// Read a file, naming it in whatever the filesystem says about the read.
pub fn read_file(path: &Path) -> Result<String, SpythonError> {
    std::fs::read_to_string(path).map_err(|err| SpythonError::FileRead {
        path: path.to_path_buf(),
        err,
    })
}
