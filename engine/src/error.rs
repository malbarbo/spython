use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use crate::check::{TypeErrors, print_type_errors};

/// What stops a spython command. Each one ends the run with a failing status,
/// after [`show_error`] has said what it was.
pub enum SpythonError {
    /// A file the user named that could not be read, either because it is not
    /// there or because the filesystem turned the read down.
    FileRead { path: PathBuf, err: std::io::Error },

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

/// Whether what goes to stderr takes colour, which the diagnostics ask before
/// they are printed.
///
/// The page decides for itself: it renders the codes and asks for them by
/// hand.
pub fn color_stderr() -> bool {
    use_color(std::io::stderr().is_terminal())
}

/// Whether what goes to stdout takes colour, which the reader asks before it
/// highlights what the student is typing.
pub fn color_stdout() -> bool {
    use_color(std::io::stdout().is_terminal())
}

/// The one rule, for a stream that is or is not a terminal. A terminal wants
/// colour, and a redirected stream keeps the codes out of the file it is
/// being read into.
///
/// `NO_COLOR` with anything in it keeps them out of every stream, as
/// <https://no-color.org> asks, and it wins over `FORCE_COLOR`: the spec
/// gives way to an option written on the command line, and neither of these
/// is one. `FORCE_COLOR` asks for colour where no terminal is watching, which
/// is how a test reads what a student sees.
fn use_color(is_terminal: bool) -> bool {
    !env_is_set("NO_COLOR") && (env_is_set("FORCE_COLOR") || is_terminal)
}

/// Whether the environment holds `name` with something in it. An empty value
/// is not an answer: no-color.org says so of `NO_COLOR`, and `FORCE_COLOR`
/// reads the same way here.
fn env_is_set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

/// Read a file, naming it in whatever the filesystem says about the read.
pub fn read_file(path: &Path) -> Result<String, SpythonError> {
    std::fs::read_to_string(path).map_err(|err| SpythonError::FileRead {
        path: path.to_path_buf(),
        err,
    })
}
