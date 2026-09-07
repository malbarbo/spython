//! Formatting Python source, with ruff's formatter.

use ruff_python_formatter::{DocstringCode, PyFormatOptions, QuoteStyle, format_module_source};

#[cfg(not(target_arch = "wasm32"))]
use crate::{SpythonError, error::read_file, error::show_error};
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

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

/// Format the given paths, recursing into directories. With no path, format
/// what stdin holds and write it to stdout. `check` writes nothing, and fails
/// naming the files that are not formatted already.
///
/// A file the formatter turns down does not stop the others, and the run
/// fails at the end all the same.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(check: bool, paths: &[PathBuf]) -> Result<(), SpythonError> {
    if paths.is_empty() {
        return run_stdin(check);
    }
    let mut unformatted = Vec::new();
    let mut turned_down = false;
    for path in paths {
        for file in python_files(path) {
            let source = read_file(&file)?;
            let formatted = match format_source(&source) {
                Ok(formatted) => formatted,
                Err(message) => {
                    show_error(&SpythonError::Format {
                        path: file,
                        message,
                    });
                    turned_down = true;
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
    if !unformatted.is_empty() {
        return Err(SpythonError::NotFormatted { paths: unformatted });
    }
    if turned_down {
        return Err(SpythonError::FormatFailed);
    }
    Ok(())
}

/// Every `.py` file under `path`, sorted, as a directory hands its entries
/// back in an order of its own. A path that names a file is that file.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
        path: PathBuf::from(STDIN),
        message,
    })?;
    if !check {
        print!("{formatted}");
        return Ok(());
    }
    if formatted == source {
        Ok(())
    } else {
        Err(SpythonError::NotFormatted {
            paths: vec![PathBuf::from(STDIN)],
        })
    }
}

/// What the messages call the text that came in over stdin.
#[cfg(not(target_arch = "wasm32"))]
const STDIN: &str = "<stdin>";
