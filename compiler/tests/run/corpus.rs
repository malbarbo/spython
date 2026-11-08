//! The programs of the corpus, compared with CPython.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use engine::Level;

use super::{deno, python, wasmtime};

/// Each program of the corpus that the checker accepts and the compiler
/// covers prints what CPython prints, and exits as CPython exits, in
/// wasmtime and in deno. The
/// corpus is `cli/tests/inputs` and the `na-programacao` beside this
/// repository, when it is there.
#[test]
fn the_corpus_runs_as_in_cpython() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = python_files(&root.join("cli/tests/inputs"));
    let book = root.join("../na-programacao");
    if book.is_dir() {
        files.extend(python_files(&book));
    } else {
        eprintln!("{} is missing, so the corpus leaves it out", book.display());
    }

    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(file) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let outcome = run_from_corpus(file);
                    outcomes
                        .lock()
                        .expect("no panic with the lock")
                        .push((file, outcome));
                }
            });
        }
    });
    let mut outcomes = outcomes.into_inner().expect("no panic with the lock");
    outcomes.sort();
    let count = |wanted: Outcome| outcomes.iter().filter(|(_, o)| *o == wanted).count();
    eprintln!(
        "corpus: {} files, {} rejected by the checker, {} unsupported, {} same as CPython",
        files.len(),
        count(Outcome::Rejected),
        count(Outcome::Unsupported),
        count(Outcome::Same),
    );
    let differ: Vec<_> = outcomes
        .iter()
        .filter(|(_, outcome)| *outcome == Outcome::Differs)
        .map(|(file, _)| file)
        .collect();
    assert!(differ.is_empty(), "differ from CPython: {differ:#?}");
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Rejected,
    Unsupported,
    Same,
    Differs,
}

/// Checks, compiles and runs `file`, and runs it in CPython too.
fn run_from_corpus(file: &Path) -> Outcome {
    let Ok(checked) = engine::check::check_file_keeping_db(file, Level::Full) else {
        return Outcome::Rejected;
    };
    let path = file.to_str().expect("a path in utf-8");
    let module = match compiler::compile(&checked.db, checked.file, path) {
        Ok(module) => module,
        Err(compiler::Error::Unsupported { .. }) => return Outcome::Unsupported,
        Err(err) => panic!("{path}: {err:?}"),
    };
    let name = format!("corpus_{}", path.replace(['/', '.'], "_"));
    let ours = wasmtime(&name, &module, None);
    let theirs = python(file);
    if (&ours.stdout, &ours.stderr, ours.status.code())
        == (&theirs.stdout, &theirs.stderr, theirs.status.code())
        && deno(&name) == ours
    {
        Outcome::Same
    } else {
        Outcome::Differs
    }
}

/// The `.py` files under `dir`, in a fixed order.
fn python_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("a directory of the corpus") {
            let path = entry.expect("an entry of the corpus").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|extension| extension == "py") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
