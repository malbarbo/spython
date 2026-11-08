//! Variables and assignments.

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn a_variable_holds_an_int_a_bool_or_a_str() {
    same_as_cpython(
        "a_variable_holds_an_int_a_bool_or_a_str",
        "n = 7\n\
         s = 'olá'\n\
         b = n > 3\n\
         print(n, s, b)\n\
         n = n * 2\n\
         s = 'mundo'\n\
         print(n, s, b, n // 4)\n\
         t: int = -3\n\
         u: str\n\
         print(t // 2, t % 2)\n",
    );
}

#[test]
fn a_chained_assignment_gives_the_value_to_every_target() {
    same_as_cpython(
        "a_chained_assignment_gives_the_value_to_every_target",
        "a = b = c = 1 + 2\nprint(a, b, c)\na = 0\nprint(a, b, c)\n",
    );
}

#[test]
fn an_augmented_assignment_follows_python() {
    same_as_cpython(
        "an_augmented_assignment_follows_python",
        "n = 10\nn += 5\nn -= 1\nn *= 3\nprint(n)\nn //= 4\nprint(n)\nn %= -4\nprint(n)\n",
    );
}

#[test]
fn the_sum_of_two_strs_joins_them() {
    same_as_cpython(
        "the_sum_of_two_strs_joins_them",
        "s = 'ab' + 'çd'\nprint(s + '' + s)\nprint('' + '')\nt = 'x'\nt += s\nt += t\nprint(t)\n",
    );
}

#[test]
fn an_augmented_assignment_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "an_augmented_assignment_by_zero_raises_zero_division_error",
        "n = 7\nn //= 0  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn an_augmented_assignment_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "an_augmented_assignment_outside_31_bits_stops_the_program",
        "n = 1073741823\nprint(n)\nn += 1\n",
        "1073741823\n",
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
        "print(1)\nprint(2, x)  # ty: ignore[unresolved-reference]\nx = 3\n",
    );
}

#[test]
fn an_augmented_assignment_before_the_assignment_raises_name_error() {
    same_as_cpython(
        "an_augmented_assignment_before_the_assignment_raises_name_error",
        "s = 1\ns += n  # ty: ignore[unresolved-reference]\nn = 3\n",
    );
}

#[test]
fn a_read_before_the_assignment_of_a_builtin_name_is_unsupported() {
    assert_eq!(
        unsupported("print(abs)  # ty: ignore[unresolved-reference]\nabs = 3\n"),
        "a name that may be a builtin"
    );
}

#[test]
fn a_variable_with_values_of_two_types_is_unsupported() {
    assert_eq!(
        unsupported("x = 1\nx = 'a'\n"),
        "a variable with values of two types"
    );
}

#[test]
fn a_variable_of_none_is_unsupported() {
    assert_eq!(unsupported("x = print(1)\n"), "a variable of this type");
}
