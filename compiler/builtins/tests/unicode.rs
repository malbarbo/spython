use std::process::Command;

use builtins::unicode::{
    str_fold_at, str_is_case_ignorable, str_is_cased, str_is_lowercase, str_is_printable,
    str_is_uppercase, str_lower_at, str_title_at, str_upper_at,
};

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

/// Compares `mapping` with `method` of `str` of CPython on every code
/// point, where `mapping` gives each code point of the result, and -1 after
/// the last one.
fn same_mapping_as_cpython(mapping: extern "C" fn(u32, u32) -> i32, method: &str) {
    let program = format!(
        "import sys\n\
         sys.stdout.write('\\n'.join(' '.join(str(ord(x)) for x in chr(c).{method}()) \
         for c in range(0x110000)))"
    );
    let output = Command::new(std::env::var("PYTHON").unwrap_or_else(|_| "python3".into()))
        .args(["-c", &program])
        .output()
        .expect("python3 should run");
    assert!(output.status.success(), "{output:?}");
    let expected = String::from_utf8(output.stdout).expect("digits are ASCII");
    let mut lines = expected.lines();
    let mut mismatches = Vec::new();
    for cp in 0..0x11_0000 {
        let got: Vec<String> = (0..4)
            .map(|i| mapping(cp, i))
            .take_while(|to| *to >= 0)
            .map(|to| to.to_string())
            .collect();
        let got = got.join(" ");
        let want = lines.next().expect("one line for each code point");
        if got != want && mismatches.len() < 20 {
            mismatches.push(format!("U+{cp:04X}: {got} != {want}"));
        }
    }
    assert!(mismatches.is_empty(), "{method}: {mismatches:?}");
}

#[test]
fn upper_matches_cpython() {
    same_mapping_as_cpython(str_upper_at, "upper");
}

#[test]
fn lower_matches_cpython() {
    same_mapping_as_cpython(str_lower_at, "lower");
}

/// `str.lower` of CPython gives the final sigma for a capital sigma after a
/// code point that is cased and not case ignorable, and after an `a` and a
/// code point that is cased or case ignorable. The two tell whether a code
/// point is case ignorable, and whether one that is not is cased, which is
/// all that `str.lower` asks.
#[test]
fn cased_and_case_ignorable_match_cpython() {
    extern "C" fn cased_alone(cp: u32) -> u32 {
        u32::from(str_is_cased(cp) == 1 && str_is_case_ignorable(cp) == 0)
    }
    extern "C" fn cased_or_ignorable(cp: u32) -> u32 {
        u32::from(str_is_cased(cp) == 1 || str_is_case_ignorable(cp) == 1)
    }
    same_as_cpython(cased_alone, "(c + '\\u03a3').lower().endswith('\\u03c2')");
    same_as_cpython(
        cased_or_ignorable,
        "('a' + c + '\\u03a3').lower().endswith('\\u03c2')",
    );
}

#[test]
fn title_matches_cpython() {
    same_mapping_as_cpython(str_title_at, "title");
}

#[test]
fn is_uppercase_and_is_lowercase_match_cpython() {
    same_as_cpython(str_is_uppercase, "c.isupper()");
    same_as_cpython(str_is_lowercase, "c.islower()");
}

#[test]
fn casefold_matches_cpython() {
    same_mapping_as_cpython(str_fold_at, "casefold");
}
