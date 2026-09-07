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
    let mut rejected = Vec::new();
    let mut unformatted = Vec::new();
    for path in paths {
        for file in python_files(path) {
            let source = read_file(&file)?;
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
fn python_files(path: &Path) -> impl Iterator<Item = PathBuf> {
    walkdir::WalkDir::new(path)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "py"))
        .map(|entry| entry.into_path())
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
