//! Formatting the files on disk that `spython format` names.

use std::path::{Path, PathBuf};

use crate::{SpythonError, error::read_file};

use super::format_source;

/// Format the given paths, recursing into directories. With no path, format
/// what stdin holds and write it to stdout. `check` writes nothing, and fails
/// naming the files that are not formatted already.
///
/// A file the formatter turns down does not stop the others, and the run
/// fails at the end all the same.
pub fn run(check: bool, paths: &[PathBuf]) -> Result<(), SpythonError> {
    if paths.is_empty() {
        return run_stdin(check);
    }
    // Every path is walked and every file read before any file is written,
    // so a path that does not exist or a file that cannot be read leaves the
    // others as they were.
    let mut files = Vec::new();
    for path in paths {
        for file in python_files(path)? {
            let source = read_file(&file)?;
            files.push((file, source));
        }
    }
    let mut rejected = Vec::new();
    let mut unformatted = Vec::new();
    for (file, source) in files {
        let formatted = match format_source(&source) {
            Ok(formatted) => formatted,
            Err(message) => {
                rejected.push((file, message));
                continue;
            }
        };
        if formatted == source {
            continue;
        }
        if check {
            unformatted.push(file);
        } else {
            std::fs::write(&file, formatted)
                .map_err(|err| SpythonError::FileWrite { path: file, err })?;
        }
    }
    if rejected.is_empty() && unformatted.is_empty() {
        Ok(())
    } else {
        Err(SpythonError::Format {
            rejected,
            unformatted,
        })
    }
}

/// Every `.py` file under `path`, sorted, as a directory hands its entries
/// back in an order of its own. A path that names a file is that file.
///
/// A path that does not exist, or a directory that cannot be read, is an
/// error, so the command never succeeds on files it did not see.
fn python_files(path: &Path) -> Result<Vec<PathBuf>, SpythonError> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry.map_err(|err| SpythonError::FileRead {
            path: err.path().unwrap_or(path).to_path_buf(),
            err: err
                .into_io_error()
                .expect("a walk that does not follow links finds no loop"),
        })?;
        // A file the command line names is formatted whatever its name, as
        // `run` and `check` take it.
        let named = entry.depth() == 0 && entry.file_type().is_file();
        // A directory, or a link to one, can have a name that ends in `.py`.
        let python =
            entry.path().extension().is_some_and(|ext| ext == "py") && entry.path().is_file();
        if named || python {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// The stdin half of [`run`], which answers on stdout instead of writing over
/// what it read.
fn run_stdin(check: bool) -> Result<(), SpythonError> {
    use std::io::Read as _;

    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .map_err(|err| SpythonError::FileRead {
            path: PathBuf::from(STDIN),
            err,
        })?;
    let formatted = format_source(&source).map_err(|message| SpythonError::Format {
        rejected: vec![(PathBuf::from(STDIN), message)],
        unformatted: Vec::new(),
    })?;
    if !check {
        print!("{formatted}");
        return Ok(());
    }
    if formatted == source {
        Ok(())
    } else {
        Err(SpythonError::Format {
            rejected: Vec::new(),
            unformatted: vec![PathBuf::from(STDIN)],
        })
    }
}

/// What the messages call the text that came in over stdin.
const STDIN: &str = "<stdin>";
