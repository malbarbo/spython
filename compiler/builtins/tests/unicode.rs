use std::process::Command;

use builtins::unicode::str_is_printable;

/// What `python` prints: for each code point, in order, `1` when
/// `expression` is true of the `str` `c` of that code point, and `0`
/// otherwise.
fn cpython(expression: &str) -> String {
    let program = format!(
        "import sys\n\
         sys.stdout.write(''.join('1' if {expression} else '0' \
         for c in map(chr, range(0x110000))))"
    );
    let output = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args(["-c", &program])
        .output()
        .expect("python3 should run");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).expect("digits are ASCII")
}

/// Compares `property` with `expression` of CPython on every code point.
fn same_as_cpython(property: extern "C" fn(u32) -> u32, expression: &str) {
    let expected = cpython(expression);
    assert_eq!(expected.len(), 0x11_0000);
    let mismatches: Vec<String> = (0..0x11_0000)
        .zip(expected.bytes())
        .filter(|(cp, want)| property(*cp) != u32::from(*want - b'0'))
        .map(|(cp, _)| format!("U+{cp:04X}"))
        .take(20)
        .collect();
    assert!(mismatches.is_empty(), "{expression}: {mismatches:?}");
}

#[test]
fn is_printable_matches_cpython() {
    same_as_cpython(str_is_printable, "c.isprintable()");
}
