//! What a run reaches on a machine with a file system: the file the student
//! named, and the files the `test` command tests.

use std::path::{Path, PathBuf};

use crate::{Level, SpythonError, check, error::read_file};

use super::{DOCTEST_RUNNER, TEST_NAME, run_source, run_source_with_argv};

/// A file a run starts from, and everything running it takes.
pub struct Script {
    /// What the file holds.
    pub source: String,
    /// The name it runs under, which is the path as the user wrote it.
    pub name: String,
    /// The directory its imports resolve from.
    pub dir: String,
    /// The first-party files it imports, each a path relative to `dir` and
    /// its text.
    pub modules: Vec<(String, String)>,
}

/// Check `file` at `level` and read it.
///
/// Both a script and a session that starts from a file come through here, so
/// the two agree on what the file is and where it looks for its imports.
pub fn check_and_read(file: &Path, level: Level) -> Result<Script, SpythonError> {
    let modules = check::check_file(file, level)?;
    Ok(Script {
        source: read_file(file)?,
        name: file.to_string_lossy().into_owned(),
        dir: parent_dir(file),
        modules,
    })
}

/// The directory a file's imports resolve from, which is the one it sits in,
/// and the working directory for a bare name.
fn parent_dir(file: &Path) -> String {
    match file.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_string_lossy().into_owned(),
        _ => ".".to_owned(),
    }
}

/// Type-check a Python script and run it.
pub fn run_main(file: &Path, level: Level) -> Result<(), SpythonError> {
    let script = check_and_read(file, level)?;
    run_source(&script.source, &script.name, &script.dir)
}

/// Run doctests for the given files.
pub fn run_test(files: &[PathBuf], level: Level, verbose: bool) -> Result<(), SpythonError> {
    // A file that is not there is an error, the way it is for `run`. Silently
    // reporting no failures for a name the student mistyped is worse than
    // saying the name is wrong.
    for file in files {
        check::check_file(file, level)?;
    }

    let mut argv = vec![TEST_NAME.to_owned()];
    if verbose {
        argv.push("--verbose".to_owned());
    }
    argv.extend(files.iter().map(|file| file.to_string_lossy().into_owned()));

    let script = format!("{DOCTEST_RUNNER}\n{TEST_MAIN}");
    if run_source_with_argv(&script, TEST_NAME, ".", argv).is_err() {
        return Err(SpythonError::TestsFailed);
    }
    Ok(())
}

/// What drives the runner: the files come in through `sys.argv`, so a path
/// with a quote or a backslash in it needs no escaping here.
const TEST_MAIN: &str = "import sys\nsys.exit(_main(sys.argv[1:]))\n";
