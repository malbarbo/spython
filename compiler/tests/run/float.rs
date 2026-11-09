//! The `float` literals, and the `float` in variables, parameters and
//! results.

use std::path::Path;

use indoc::indoc;
use wasmparser::{DataKind, Parser, Payload};

use super::{compile, same_as_cpython, unsupported};

#[test]
fn print_writes_the_repr_of_a_float() {
    same_as_cpython(
        "print_writes_the_repr_of_a_float",
        indoc! {"
            print(2.5, 0.0, 0.1, 100.0, 123456789.0, 0.30000000000000004)
            print(1e16, 1e15, 1e-05, 0.0001, 1e22, 1.5e300, 5e-324, 1e309)
            print('x =', 1.5, 2)
        "},
    );
}

#[test]
fn a_float_goes_through_variables_and_functions() {
    same_as_cpython(
        "a_float_goes_through_variables_and_functions",
        indoc! {"
            def same(x: float) -> float:
                y = x
                return y

            z = 2.5
            w: float = 0.1
            print(same(z), z, w)
        "},
    );
}

#[test]
fn the_truth_of_a_float_is_unsupported() {
    for source in [
        indoc! {"
            if 1.5:
                print(1)
        "},
        indoc! {"
            print(1.5 or 2.5)
        "},
    ] {
        assert_eq!(unsupported(source), "the truth of a float");
    }
}

#[test]
fn an_operator_on_a_float_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print(1.5 + 1)
        "}),
        "an operator on a value that is not an int"
    );
}

/// A program without a `float` leaves out `f64_repr` and its data, which
/// are the only active segments.
#[test]
fn a_program_without_a_float_links_no_builtin() {
    let module = compile("print(1)\n", Path::new("no_float.py")).expect("the program to compile");
    for payload in Parser::new(0).parse_all(&module.bytes) {
        if let Payload::DataSection(s) = payload.expect("a valid module") {
            for data in s {
                let data = data.expect("a segment");
                assert!(matches!(data.kind, DataKind::Passive), "an active segment");
            }
        }
    }
}
