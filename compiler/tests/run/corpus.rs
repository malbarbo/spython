//! The programs of the corpus, compared with CPython.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use engine::Level;

use super::doctests::{docstrings, runner};
use super::{deno, python, wasmtime};

/// Each program of the corpus that the checker accepts at the level of its
/// chapter and the compiler covers prints what CPython prints, and exits as
/// CPython exits, in wasmtime and in deno.
#[test]
fn the_corpus_runs_as_in_cpython() {
    let files = corpus();
    let outcomes = each_file(&files, run_from_corpus);
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
    assert_covered(&outcomes);
}

/// The files of the corpus, which is `cli/tests/inputs` and the
/// `na-programacao` beside this repository, when it is there.
fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = python_files(&root.join("cli/tests/inputs"));
    let book = book();
    if book.is_dir() {
        files.extend(python_files(&book));
    } else {
        eprintln!("{} is missing, so the corpus leaves it out", book.display());
    }
    files
}

/// The `na-programacao` beside this repository.
fn book() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../na-programacao")
}

/// The level of each chapter of `na-programacao` that the compiler covers.
const CHAPTERS: &[(&str, Level)] = &[
    ("01-algoritmos-e-linguagens", Level::Functions),
    ("02-conceitos-basicos", Level::Functions),
    ("03-projeto-de-funcoes", Level::Functions),
    ("04-selecao", Level::Selection),
    ("05-tipos-de-dados", Level::UserTypes),
];

/// The level of the chapter of `file`, and the full level for a file
/// outside those chapters.
fn level(file: &Path) -> Level {
    let chapter = file
        .strip_prefix(book())
        .ok()
        .and_then(|path| path.components().next())
        .map(|chapter| chapter.as_os_str());
    CHAPTERS
        .iter()
        .find(|(name, _)| chapter == Some(name.as_ref()))
        .map_or(Level::Full, |(_, level)| *level)
}

/// The outcome of `run` for each of `files`, in the order of the files.
fn each_file(files: &[PathBuf], run: fn(&Path) -> Outcome) -> Vec<(&PathBuf, Outcome)> {
    let next = AtomicUsize::new(0);
    let outcomes = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(file) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let outcome = run(file);
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
    outcomes
}

/// The doctests of each file of the corpus that the checker accepts and the
/// compiler covers write the report of the runner of spython, and exit as
/// it exits, in wasmtime and in deno. A file where an example leaves the
/// 31 bits of an `int` stops there, and the runner goes on, so the file is
/// out of the comparison.
#[test]
fn the_doctests_of_the_corpus_run_as_in_the_runner() {
    let files = corpus();
    let outcomes = each_file(&files, tests_from_corpus);
    let count = |wanted: Outcome| outcomes.iter().filter(|(_, o)| *o == wanted).count();
    eprintln!(
        "doctests of the corpus: {} files, {} rejected by the checker, {} unsupported, \
         {} past the limit of an int, {} same as the runner",
        files.len(),
        count(Outcome::Rejected),
        count(Outcome::Unsupported),
        count(Outcome::PastLimit),
        count(Outcome::Same),
    );
    let differ: Vec<_> = outcomes
        .iter()
        .filter(|(_, outcome)| *outcome == Outcome::Differs)
        .map(|(file, _)| file)
        .collect();
    assert!(differ.is_empty(), "differ from the runner: {differ:#?}");
    assert_covered(&outcomes);
}

/// The highest level whose programs the compiler covers.
const COVERED: Level = Level::UserTypes;

/// Asserts that the compiler covers each file of `outcomes` that the
/// checker accepts at a level up to [`COVERED`].
fn assert_covered(outcomes: &[(&PathBuf, Outcome)]) {
    let unsupported: Vec<_> = outcomes
        .iter()
        .filter(|(file, outcome)| *outcome == Outcome::Unsupported && level(file) <= COVERED)
        .map(|(file, _)| file)
        .collect();
    assert!(
        unsupported.is_empty(),
        "unsupported at a covered level: {unsupported:#?}"
    );
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Rejected,
    Unsupported,
    /// The program stopped at the limit of an `int`.
    PastLimit,
    Same,
    Differs,
}

/// Checks `file`, compiles its doctests and runs them, and runs them with
/// the runner of spython too.
fn tests_from_corpus(file: &Path) -> Outcome {
    let Ok(checked) = engine::check::check_file_keeping_db(file, level(file)) else {
        return Outcome::Rejected;
    };
    let path = file.to_str().expect("a path in utf-8");
    let program = checked.into_doctests();
    let docstrings = docstrings(program.docstrings);
    let module = match compiler::compile_tests(
        &program.db,
        program.file,
        path,
        &docstrings,
        &program.copies,
        false,
    ) {
        Ok(module) => module,
        Err(compiler::Error::Unsupported { .. }) => return Outcome::Unsupported,
        Err(err) => panic!("{path}: {err:?}"),
    };
    let name = format!("corpus_tests_{}", path.replace(['/', '.'], "_"));
    let ours = wasmtime(&name, &module, None);
    if String::from_utf8_lossy(&ours.stderr).contains("\nspython: the result ") {
        return Outcome::PastLimit;
    }
    let source = std::fs::read_to_string(file).expect("a file of the corpus");
    let theirs = runner(&name, file, &source, false);
    if (&ours.stdout, &ours.stderr, ours.status.code())
        == (
            &theirs.stdout,
            &theirs.stderr,
            theirs.status.code().map(|code| code.min(1)),
        )
        && deno(&name) == ours
    {
        Outcome::Same
    } else {
        Outcome::Differs
    }
}

/// Checks, compiles and runs `file`, and runs it in CPython too.
fn run_from_corpus(file: &Path) -> Outcome {
    let Ok(checked) = engine::check::check_file_keeping_db(file, level(file)) else {
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
