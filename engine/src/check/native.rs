//! Checking a file on disk: the database it builds around the file's own
//! directory, and the check the `run`, `test` and `check` commands run.

use std::collections::HashSet;
use std::path::Path;

use ruff_db::files::system_path_to_file;
use ruff_db::system::SystemPathBuf;
use ty_project::{Db as _, ProjectDatabase};

use crate::error::color_stderr;
use crate::{Level, SpythonError};

use super::{
    TypeErrors, annotation_check, check_file_doctests, collect_import_files, make_project_metadata,
    modules_beside, print_type_errors, user_files,
};

/// The spython lib, served from memory to a checker that looks on disk for it.
mod overlay;

/// Type-check a Python file: validates the path, builds the ty database,
/// runs the annotation checker, then runs ty's type checker.
///
/// Returns the first-party files it imports, each a path relative to its own
/// directory and its text, for a REPL that has to keep checking against them.
pub fn check_file(file: &Path, level: Level) -> Result<Vec<(String, String)>, SpythonError> {
    use ruff_db::diagnostic::Severity;

    let abs_file = dunce::canonicalize(file).map_err(|err| SpythonError::FileRead {
        path: file.to_path_buf(),
        err,
    })?;
    if !abs_file.is_file() {
        return Err(SpythonError::PathIsNotAFile {
            path: file.to_path_buf(),
        });
    }

    let abs_sys = SystemPathBuf::from_path_buf(abs_file)
        .map_err(|path| SpythonError::NonUtf8Path { path })?;
    // The file's own directory, not the process's: `import helpers` looks
    // beside the file that wrote it, which is also where the interpreter
    // looks when it runs (`sys.path[0]`). Rooting the check anywhere else
    // makes `spython test dir/main.py` disagree with `cd dir` for no reason
    // the student can see.
    let root = abs_sys.parent().unwrap_or(&abs_sys).to_path_buf();
    let mut db = build_db(root.as_std_path())?;
    let main_file =
        system_path_to_file(&db, &abs_sys).map_err(|e| SpythonError::Database(e.to_string()))?;

    let mut files = HashSet::new();
    files.insert(main_file);
    collect_import_files(&db, main_file, &mut files);

    let sys_files: Vec<SystemPathBuf> = files
        .iter()
        .filter_map(|f| f.path(&db).as_system_path().map(|p| p.to_path_buf()))
        .collect();
    db.project().set_included_paths(&mut db, sys_files);

    let mut diagnostics = annotation_check(&db, level, false);
    diagnostics.extend(db.check());
    // Validate and type-check doctests inside each user file's docstrings.
    let user_files = user_files(&db, files.iter().copied());
    for f in &user_files {
        diagnostics.extend(check_file_doctests(&db, *f, level));
    }
    // An error stops the file; a warning is printed and the file goes on. The
    // in-memory check the page runs has always drawn the line here, and this is
    // where the one over the filesystem draws it too.
    if diagnostics.iter().any(|d| d.severity() == Severity::Error) {
        return Err(SpythonError::TypeErrors(Box::new(TypeErrors {
            db,
            diagnostics,
        })));
    }
    print_type_errors(&db, &diagnostics, color_stderr());

    Ok(modules_beside(&db, main_file, user_files))
}

/// Build a ty ProjectDatabase for the given file.
fn build_db(cwd: &Path) -> Result<ProjectDatabase, SpythonError> {
    let cwd_sys = SystemPathBuf::from_path_buf(cwd.to_path_buf())
        .map_err(|path| SpythonError::NonUtf8Path { path })?;

    let (system, lib_root) = overlay::OverlaySystem::new(&cwd_sys);
    let metadata = make_project_metadata(cwd_sys, vec![lib_root]);

    ProjectDatabase::fallible(metadata, system).map_err(|e| SpythonError::Database(e.to_string()))
}
