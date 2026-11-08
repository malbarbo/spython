//! Variables and assignments.

use indoc::indoc;

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn a_variable_holds_an_int_a_bool_or_a_str() {
    same_as_cpython(
        "a_variable_holds_an_int_a_bool_or_a_str",
        indoc! {"
            n = 7
            s = 'olá'
            b = n > 3
            print(n, s, b)
            n = n * 2
            s = 'mundo'
            print(n, s, b, n // 4)
            t: int = -3
            u: str
            print(t // 2, t % 2)
        "},
    );
}

#[test]
fn a_chained_assignment_gives_the_value_to_every_target() {
    same_as_cpython(
        "a_chained_assignment_gives_the_value_to_every_target",
        indoc! {"
            a = b = c = 1 + 2
            print(a, b, c)
            a = 0
            print(a, b, c)
        "},
    );
}

#[test]
fn an_augmented_assignment_follows_python() {
    same_as_cpython(
        "an_augmented_assignment_follows_python",
        indoc! {"
            n = 10
            n += 5
            n -= 1
            n *= 3
            print(n)
            n //= 4
            print(n)
            n %= -4
            print(n)
        "},
    );
}

#[test]
fn the_sum_of_two_strs_joins_them() {
    same_as_cpython(
        "the_sum_of_two_strs_joins_them",
        indoc! {"
            s = 'ab' + 'çd'
            print(s + '' + s)
            print('' + '')
            t = 'x'
            t += s
            t += t
            print(t)
        "},
    );
}

#[test]
fn an_augmented_assignment_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "an_augmented_assignment_by_zero_raises_zero_division_error",
        indoc! {"
            n = 7
            n //= 0  # ty: ignore[division-by-zero]
        "},
    );
}

#[test]
fn an_augmented_assignment_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "an_augmented_assignment_outside_31_bits_stops_the_program",
        indoc! {"
            n = 1073741823
            print(n)
            n += 1
        "},
        indoc! {"
            1073741823
        "},
        3,
        "    n += 1\n",
        "1073741824",
    );
}

// The checker rejects a read before the assignment, unless a comment
// silences the error.
#[test]
fn a_read_before_the_assignment_raises_name_error() {
    same_as_cpython(
        "a_read_before_the_assignment_raises_name_error",
        indoc! {"
            print(1)
            print(2, x)  # ty: ignore[unresolved-reference]
            x = 3
        "},
    );
}

#[test]
fn an_augmented_assignment_before_the_assignment_raises_name_error() {
    same_as_cpython(
        "an_augmented_assignment_before_the_assignment_raises_name_error",
        indoc! {"
            s = 1
            s += n  # ty: ignore[unresolved-reference]
            n = 3
        "},
    );
}

#[test]
fn a_read_before_the_assignment_of_a_builtin_name_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print(abs)  # ty: ignore[unresolved-reference]
            abs = 3
        "}),
        "a name that may be a builtin"
    );
}

#[test]
fn a_variable_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            x = 1
            x = 'a'
        "}),
        "a variable with values of two types"
    );
}

#[test]
fn a_variable_of_none_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
        x = print(1)
    "}),
        "a variable of this type"
    );
}
