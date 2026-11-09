//! The `float` literals, the `float` in variables, parameters and results,
//! and the operators on a `float`.

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
fn add_sub_and_mul_on_floats() {
    same_as_cpython(
        "add_sub_and_mul_on_floats",
        indoc! {"
            def area(w: float, h: float) -> float:
                return w * h

            x = 0.1
            print(x + 0.2, 2.5 - x, area(1.5, x), 1e308 * 10, -1e308 * 10)
            big = 1e308 * 10
            print(big - big, 0.1 * 3 - 0.3)
            y = 1.5
            y += 1
            y -= 0.25
            y *= 2
            print(y)
        "},
    );
}

#[test]
fn an_int_or_a_bool_mixes_with_a_float() {
    same_as_cpython(
        "an_int_or_a_bool_mixes_with_a_float",
        indoc! {"
            n = 1073741823
            print(n * 1.5, 1.5 * n, -1073741824 + 0.5, 3 - 0.5)
            print(True + 1.5, 1.5 - False, True * 0.5)
        "},
    );
}

#[test]
fn the_sign_of_a_float_follows_python() {
    same_as_cpython(
        "the_sign_of_a_float_follows_python",
        indoc! {"
            x = 0.0
            print(-x, 0 - x, -x + 0.0, -x - 0.0, -x * 1, +x, -(-x))
            y = 2.5
            print(-y, +y, --y, -1.5, -0 * -1.5)
        "},
    );
}

#[test]
fn the_true_division_gives_a_float() {
    same_as_cpython(
        "the_true_division_gives_a_float",
        indoc! {"
            print(7 / 2, -7 / 2, 1 / 3, 2 / 1, 1073741823 / 7, -1073741824 / -1)
            print(0 / 5, 0 / -5, -0 / 5, True / 2, 3 / True)
            print(1.5 / 0.5, 1 / 3.0, 0.1 / 3, -0.0 / 1, 0.0 / -2.5, 1e308 / 1e-10)
            x = 5.0
            x /= 4
            print(x)
        "},
    );
}

#[test]
fn the_true_division_of_ints_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_ints_by_zero_raises_zero_division_error",
        indoc! {"
            print(1 / 2)
            print(7 / 0)  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn the_true_division_of_bools_by_false_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_bools_by_false_raises_zero_division_error",
        indoc! {"
            def ratio(a: bool, b: bool) -> float:
                return a / b

            print(ratio(True, True))
            print(ratio(True, False))
        "},
    );
}

#[test]
fn the_true_division_by_a_float_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_by_a_float_zero_raises_zero_division_error",
        indoc! {"
            def inverse(x: float) -> float:
                return 1 / x

            print(inverse(2.0))
            print(inverse(-0.0))
        "},
    );
}

#[test]
fn the_true_division_of_a_float_by_an_int_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_true_division_of_a_float_by_an_int_zero_raises_zero_division_error",
        indoc! {"
            def scale(x: float, n: int) -> float:
                x /= n
                return x

            print(scale(2.5, 2))
            print(scale(2.5, 0))
        "},
    );
}

#[test]
fn floor_div_and_mod_on_a_float_are_unsupported() {
    for source in [
        indoc! {"
            print(1.5 // 1)
        "},
        indoc! {"
            print(3 % 1.5)
        "},
    ] {
        assert_eq!(unsupported(source), "this operator on a float");
    }
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
