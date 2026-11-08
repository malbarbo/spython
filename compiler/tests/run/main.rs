//! Each test compiles a program, runs it in wasmtime and in CPython, and
//! compares what the two print and how they exit.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use engine::Level;
use engine::check::check_source;

mod control;
mod corpus;
mod functions;
mod int;
mod memory;
mod name_error;
mod print;
mod recursion;
mod str;
mod vars;

/// The module of `source`, which has to pass the checker at the last level.
/// A traceback names the file by `path`.
fn compile(source: &str, path: &Path) -> Result<compiler::Module, compiler::Error> {
    let checked = match check_source(source, Level::Full, false) {
        Ok(checked) => checked,
        Err(errors) => panic!("the checker rejects the program: {:?}", errors.diagnostics),
    };
    let path = path.to_str().expect("a path in utf-8");
    compiler::compile(&checked.db, checked.file, path)
}

/// The path of a file for the test `name`. Tests run at the same time, so
/// each one has its own.
fn path(name: &str, extension: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.{extension}"))
}

/// Runs `module` in the wasmtime on the `PATH`, or in the one that
/// `WASMTIME` names. The stack of 8 MiB holds 1000 frames of the usual
/// size, the recursion limit of CPython, and a backtrace of 2000 frames
/// holds them too. The GC heap has `heap` bytes, when it is given. A
/// program that runs out of the heap writes the traceback of
/// [`compiler::Module::memory_error`], and one that fills the stack writes
/// the traceback of [`compiler::Module::recursion_error`]. Both exit with
/// 1, as the CLI does.
fn wasmtime(name: &str, module: &compiler::Module, heap: Option<u32>) -> Output {
    let mut output = output_within_limit(wasmtime_command(name, module, heap));
    let stderr = String::from_utf8_lossy(&output.stderr);
    if let Some(traceback) = module
        .memory_error(&stderr)
        .or_else(|| module.recursion_error(&stderr))
    {
        output.stderr = traceback.into_bytes();
        output.status = exit_status_1();
    }
    output
}

/// The status of a process that exited with 1.
fn exit_status_1() -> ExitStatus {
    #[cfg(unix)]
    let status = std::os::unix::process::ExitStatusExt::from_raw(1 << 8);
    #[cfg(windows)]
    let status = std::os::windows::process::ExitStatusExt::from_raw(1);
    status
}

/// The command that runs `module` in wasmtime, as [`wasmtime`] says.
fn wasmtime_command(name: &str, module: &compiler::Module, heap: Option<u32>) -> Command {
    let path = path(name, "wasm");
    std::fs::write(&path, &module.bytes).expect("the module written to disk");
    let wasmtime = std::env::var("WASMTIME").unwrap_or_else(|_| "wasmtime".to_owned());
    let mut command = Command::new(wasmtime);
    command
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y,max-wasm-stack=8388608",
            "-D",
            "max-backtrace=2000",
        ])
        .args(["--invoke", "_start"]);
    if let Some(heap) = heap {
        command.args([
            "-O",
            &format!(
                "gc-heap-reservation={heap},gc-heap-reservation-for-growth=0,gc-heap-may-move=n"
            ),
        ]);
    }
    command.arg(&path);
    command
}

/// Runs `source` in the python3 on the `PATH`, or in the one that `PYTHON`
/// names.
fn cpython(name: &str, source: &str) -> Output {
    let path = path(name, "py");
    std::fs::write(&path, source).expect("the program written to disk");
    python(&path)
}

/// Runs the program in `file` in the python3 on the `PATH`, or in the one
/// that `PYTHON` names.
fn python(file: &Path) -> Output {
    output_within_limit(python_command(file))
}

/// The command that runs the program in `file`, as [`python`] says.
fn python_command(file: &Path) -> Command {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let mut command = Command::new(python);
    command.arg(file);
    command
}

/// Runs `command` with the standard output and the standard error in one
/// pipe, as `2>&1` does, and returns what it wrote there.
fn merged_output(mut command: Command) -> String {
    let (mut reader, writer) = std::io::pipe().expect("a pipe");
    command
        .stdout(writer.try_clone().expect("a copy of the pipe"))
        .stderr(writer);
    let mut child = command.spawn().expect("the program to start");
    // The command holds the writers until it is dropped, and the reader
    // waits for the last writer to close.
    drop(command);
    let mut text = String::new();
    reader
        .read_to_string(&mut text)
        .expect("the output of the program");
    child.wait().expect("the program to end");
    text
}

/// Runs `command` and collects what it prints. A test fails when the
/// program runs for more than 10 seconds, which is a program that never
/// ends, so the other tests still run.
fn output_within_limit(command: Command) -> Output {
    output_with_stdout(command, Stdio::piped())
}

/// Runs `command` as [`output_within_limit`] does, with `stdout` as its
/// standard output. The output has what the program printed only when
/// `stdout` is a pipe.
fn output_with_stdout(mut command: Command, stdout: Stdio) -> Output {
    let limit = Duration::from_secs(10);
    let mut child = command
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .expect("the program to start");
    // Each pipe has its own reader, so a program that fills one pipe does
    // not stop while the test waits on the other.
    let stdout = child.stdout.take().map(read_in_thread);
    let stderr = read_in_thread(child.stderr.take().expect("a piped stderr"));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the status of the program") {
            break status;
        }
        if start.elapsed() > limit {
            child.kill().expect("the program to stop");
            child.wait().expect("the program to stop");
            panic!("{command:?} ran for more than {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: stdout.map_or_else(Vec::new, |stdout| {
            stdout.join().expect("the reader of stdout")
        }),
        stderr: stderr.join().expect("the reader of stderr"),
    }
}

fn read_in_thread(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes)
            .expect("the output of the program");
        bytes
    })
}

/// Asserts that the compiled `source` prints what CPython prints, and exits
/// as CPython exits.
fn same_as_cpython(name: &str, source: &str) {
    let ours = run(name, source);
    let theirs = cpython(name, source);
    assert_eq!(
        String::from_utf8_lossy(&ours.stdout),
        String::from_utf8_lossy(&theirs.stdout),
        "stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(&ours.stderr),
        String::from_utf8_lossy(&theirs.stderr),
        "stderr"
    );
    assert_eq!(ours.status.code(), theirs.status.code(), "exit code");
}

/// Compiles `source` and runs it in wasmtime.
/// Compiles `source` and runs it in wasmtime and in deno, which have to
/// print the same and exit the same.
fn run(name: &str, source: &str) -> Output {
    let module = compile(source, &path(name, "py")).expect("the program to compile");
    let output = wasmtime(name, &module, None);
    assert_eq!(deno(name), output, "deno and wasmtime");
    output
}

/// Runs the module of the test `name`, which [`wasmtime`] wrote, in the
/// deno on the `PATH`, or in the one that `DENO` names.
fn deno(name: &str) -> Output {
    output_within_limit(deno_command(name))
}

/// The command that runs the module of the test `name` in deno, as
/// [`deno`] says.
fn deno_command(name: &str) -> Command {
    let deno = std::env::var("DENO").unwrap_or_else(|_| "deno".to_owned());
    let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/wasi.ts");
    let mut command = Command::new(deno);
    command
        .args(["run", "--quiet", "--allow-read"])
        .arg(runner)
        .arg(path(name, "wasm"));
    command
}

/// Asserts that the compiled `source`, run in wasmtime and in deno with
/// the standard output that `stdout` gives, writes `stderr`, where `{file}`
/// stands for the path of the program, and exits with `code`.
fn fails_to_write(name: &str, source: &str, stdout: impl Fn() -> Stdio, stderr: &str, code: i32) {
    let module = compile(source, &path(name, "py")).expect("the program to compile");
    let stderr = stderr.replace("{file}", &path(name, "py").display().to_string());
    for output in [
        output_with_stdout(wasmtime_command(name, &module, None), stdout()),
        output_with_stdout(deno_command(name), stdout()),
    ] {
        assert_eq!(String::from_utf8_lossy(&output.stderr), stderr, "stderr");
        assert_eq!(output.status.code(), Some(code), "exit code");
    }
}

/// Asserts that the compiled `source`, run in wasmtime and in deno with
/// the standard output that `stdout` gives, writes to the standard error
/// what CPython writes, and exits as CPython exits.
fn writes_as_cpython(name: &str, source: &str, stdout: impl Fn() -> Stdio) {
    let file = path(name, "py");
    let module = compile(source, &file).expect("the program to compile");
    std::fs::write(&file, source).expect("the program written to disk");
    let theirs = output_with_stdout(python_command(&file), stdout());
    for ours in [
        output_with_stdout(wasmtime_command(name, &module, None), stdout()),
        output_with_stdout(deno_command(name), stdout()),
    ] {
        assert_eq!(
            String::from_utf8_lossy(&ours.stderr),
            String::from_utf8_lossy(&theirs.stderr),
            "stderr"
        );
        assert_eq!(ours.status.code(), theirs.status.code(), "exit code");
    }
}

/// Asserts that the compiled `source`, run in wasmtime and in deno with
/// both outputs in one pipe, writes there what CPython writes.
fn merges_as_cpython(name: &str, source: &str) {
    let file = path(name, "py");
    let module = compile(source, &file).expect("the program to compile");
    std::fs::write(&file, source).expect("the program written to disk");
    let theirs = merged_output(python_command(&file));
    assert_eq!(
        merged_output(wasmtime_command(name, &module, None)),
        theirs,
        "wasmtime"
    );
    assert_eq!(merged_output(deno_command(name)), theirs, "deno");
}

/// Asserts that the compiled `source` prints `stdout`, then stops with the
/// error of an `int` outside 31 bits at `line`, with `lines` under the
/// `File` line of the traceback.
fn stops_at_the_int_limit(
    name: &str,
    source: &str,
    stdout: &str,
    line: u32,
    lines: &str,
    result: &str,
) {
    let output = run(name, source);
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "stdout");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  \
             File \"{}\", line {line}, in <module>\n{lines}\
             spython: the result {result} does not fit in an int, \
             which goes from -1073741824 to 1073741823\n",
            path(name, "py").display()
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

/// Asserts that the compiled `source`, with a GC heap of 64 MiB, stops
/// with a `MemoryError` whose traceback has `frames`, where `{file}` stands
/// for the path of the program. CPython writes the same traceback when it
/// runs out of memory at the same place, as a run under `ulimit -v` showed
/// once. wasmtime ends the program with what it printed still in the
/// buffers of the module, so the standard output loses it.
fn runs_out_of_memory(name: &str, source: &str, frames: &str) {
    let module = compile(source, &path(name, "py")).expect("the program to compile");
    let output = wasmtime(name, &module, Some(64 << 20));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "", "stdout");
    let file = path(name, "py");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n{}MemoryError\n",
            frames.replace("{file}", &file.display().to_string())
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

fn unsupported(source: &str) -> String {
    match compile(source, Path::new("unsupported.py")) {
        Err(compiler::Error::Unsupported { what, .. }) => what,
        Err(error) => panic!("an invalid module: {error:?}"),
        Ok(_) => panic!("the program compiled"),
    }
}
