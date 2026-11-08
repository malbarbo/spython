//! The recursion limit and the traceback of a recursion.

use indoc::formatdoc;

use super::{compile, cpython, path, same_as_cpython, wasmtime};

#[test]
fn a_recursive_function_runs_up_to_the_limit_of_cpython() {
    same_as_cpython(
        "a_recursive_function_runs_up_to_the_limit_of_cpython",
        "def count(n: int) -> int:\n    if n == 0:\n        return 0\n    return 1 + count(n - 1)\n\
         \n\
         print(count(998))\n\
         print(count(999))\n",
    );
}

#[test]
fn a_recursion_without_end_raises_recursion_error() {
    same_as_cpython(
        "a_recursion_without_end_raises_recursion_error",
        "def f(n: int) -> int:\n    return f(n + 1)\n\nprint(f(0))\n",
    );
}

#[test]
fn the_traceback_writes_how_many_frames_on_the_same_line_it_leaves_out() {
    for (name, n) in [
        ("the_traceback_leaves_out_no_frame", 2),
        ("the_traceback_leaves_out_one_frame", 4),
        ("the_traceback_leaves_out_two_frames", 5),
    ] {
        same_as_cpython(
            name,
            &format!(
                "def f(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return f(n - 1)\n\
                 \n\
                 print(f({n}))\n"
            ),
        );
    }
}

#[test]
fn a_mutual_recursion_has_every_frame_in_the_traceback() {
    same_as_cpython(
        "a_mutual_recursion_has_every_frame_in_the_traceback",
        "def a(n: int) -> int:\n    return b(n)\n\
         \n\
         def b(n: int) -> int:\n    if n == 0:\n        return 1 // n  # ty: ignore[division-by-zero]\n    return a(n - 1)\n\
         \n\
         print(a(3))\n",
    );
}

#[test]
fn a_recursion_that_fills_the_stack_raises_recursion_error() {
    let name = "a_recursion_that_fills_the_stack_raises_recursion_error";
    // 900 parameters make a frame too large for 1000 of them in the stack.
    let params = (0..900).map(|i| format!("p{i}: int")).collect::<Vec<_>>();
    let args = (0..900).map(|i| format!("p{i}")).collect::<Vec<_>>();
    let source = formatdoc! {"
        def f({params}) -> int:
            return f({args})

        print(f({zeros}))
        ",
        params = params.join(", "),
        args = args.join(", "),
        zeros = vec!["0"; 900].join(", "),
    };
    let module = compile(&source, &path(name, "py")).expect("the program to compile");
    let ours = wasmtime(name, &module, None);
    let theirs = cpython(name, &source);
    // The stack fills before the recursion limit, so the traceback leaves
    // out fewer frames than the one of CPython.
    assert_ne!(ours.stderr, theirs.stderr, "a stack that held the frames");
    assert_eq!(
        without_repeat_count(&String::from_utf8_lossy(&ours.stderr)),
        without_repeat_count(&String::from_utf8_lossy(&theirs.stderr)),
        "stderr"
    );
    assert_eq!(ours.status.code(), theirs.status.code(), "exit code");
}

/// `traceback` with `N` for the number of frames that each `[Previous line
/// repeated ...]` leaves out.
fn without_repeat_count(traceback: &str) -> String {
    traceback
        .lines()
        .map(
            |line| match line.strip_prefix("  [Previous line repeated ") {
                Some(_) => "  [Previous line repeated N more times]\n".to_owned(),
                None => format!("{line}\n"),
            },
        )
        .collect()
}
