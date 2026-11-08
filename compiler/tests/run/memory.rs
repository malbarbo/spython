//! The `MemoryError` of a program that runs out of memory.

use super::{path, run, runs_out_of_memory};

#[test]
fn a_str_longer_than_an_array_raises_memory_error() {
    let output = run(
        "a_str_longer_than_an_array_raises_memory_error",
        "s = 'abc'\nprint(1)\nprint(s * 1073741823)\n",
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n", "stdout");
    let path = path("a_str_longer_than_an_array_raises_memory_error", "py");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Traceback (most recent call last):\n  File \"{}\", line 3, in <module>\n    print(s * 1073741823)\n          ~~^~~~~~~~~~~~\nMemoryError\n",
            path.display()
        ),
        "stderr"
    );
    assert_eq!(output.status.code(), Some(1), "exit code");
}

#[test]
fn a_str_too_long_for_memory_raises_memory_error() {
    runs_out_of_memory(
        "a_str_too_long_for_memory_raises_memory_error",
        "print('start')\ns = 'ab' * 100_000_000\nprint(len(s))\n",
        "  File \"{file}\", line 2, in <module>\n    \
         s = 'ab' * 100_000_000\n        \
         ~~~~~^~~~~~~~~~~~~\n",
    );
}

#[test]
fn a_memory_error_has_the_frames_of_the_calls() {
    runs_out_of_memory(
        "a_memory_error_has_the_frames_of_the_calls",
        "def double(s: str) -> str:\n    return s + s\n\n\n\
         s = 'a' * 2_000_000\nt = double(s)\nprint(len(t))\nu = double(t)\nprint(len(u))\n",
        "  File \"{file}\", line 8, in <module>\n    u = double(t)\n  \
         File \"{file}\", line 2, in double\n    return s + s\n           ~~^~~\n",
    );
}

#[test]
fn a_memory_error_in_a_recursion_writes_how_many_frames_it_leaves_out() {
    let frame = "  File \"{file}\", line 2, in grow\n    return grow(s + s)\n";
    runs_out_of_memory(
        "a_memory_error_in_a_recursion_writes_how_many_frames_it_leaves_out",
        "def grow(s: str) -> str:\n    return grow(s + s)\n\n\nprint(grow('a'))\n",
        &format!(
            "  File \"{{file}}\", line 5, in <module>\n    print(grow('a'))\n          \
             ~~~~^^^^^\n{frame}{frame}{frame}  [Previous line repeated 20 more times]\n"
        ),
    );
}
