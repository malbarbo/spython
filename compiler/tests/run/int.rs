//! The operators of `int`, its limit of 31 bits, and the division by 0.

use super::{same_as_cpython, stops_at_the_int_limit, unsupported};

#[test]
fn print_writes_an_int() {
    same_as_cpython(
        "print_writes_an_int",
        "print(0, 7, -7, 10, 1073741823, -1073741824)\n",
    );
}

#[test]
fn the_operators_of_int_follow_python() {
    same_as_cpython(
        "the_operators_of_int_follow_python",
        "print(2 + 3 * 4, 7 - 10, -(3 - 5), +4, 2 * -3, 32768 * 32767)\n",
    );
}

#[test]
fn a_bool_is_an_int_in_an_operator() {
    same_as_cpython(
        "a_bool_is_an_int_in_an_operator",
        "print(True, False, True + True, -True, +False, True * 3)\n",
    );
}

#[test]
fn the_comparisons_of_int_give_a_bool() {
    same_as_cpython(
        "the_comparisons_of_int_give_a_bool",
        "print(1 < 2, 2 <= 1, 3 == 3, 3 != 3, 4 > 5, 5 >= 5, -1 < 0, False < True)\n\
         print(1 < 2 < 3, 1 < 3 < 2, 3 > 2 > 1 > 0, 1 == 1 != 2)\n",
    );
}

#[test]
fn a_chain_of_comparisons_stops_at_the_first_false() {
    same_as_cpython(
        "a_chain_of_comparisons_stops_at_the_first_false",
        "print(2 < 1 < 1073741823 + 1)\n",
    );
}

#[test]
fn an_int_expression_as_a_statement_prints_nothing() {
    same_as_cpython(
        "an_int_expression_as_a_statement_prints_nothing",
        "1 + 2\n2 < 3\nTrue\nprint(1)\n",
    );
}

#[test]
fn a_sum_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_sum_outside_31_bits_stops_the_program",
        "print('antes')\nprint(1073741823 + 1)\nprint('depois')\n",
        "antes\n",
        2,
        "    print(1073741823 + 1)\n          ~~~~~~~~~~~^~~\n",
        "1073741824",
    );
}

#[test]
fn a_difference_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_difference_outside_31_bits_stops_the_program",
        "print(-1073741824 - 1)\n",
        "",
        1,
        "    print(-1073741824 - 1)\n          ~~~~~~~~~~~~^~~\n",
        "-1073741825",
    );
}

#[test]
fn a_product_outside_31_bits_stops_the_program() {
    stops_at_the_int_limit(
        "a_product_outside_31_bits_stops_the_program",
        "print(65536 * 65536)\n",
        "",
        1,
        "    print(65536 * 65536)\n          ~~~~~~^~~~~~~\n",
        "4294967296",
    );
}

#[test]
fn the_negation_of_the_least_int_stops_the_program() {
    stops_at_the_int_limit(
        "the_negation_of_the_least_int_stops_the_program",
        "print(-(-1073741823 - 1))\n",
        "",
        1,
        "    print(-(-1073741823 - 1))\n          ^^^^^^^^^^^^^^^^^^\n",
        "1073741824",
    );
}

#[test]
fn an_int_literal_outside_31_bits_is_unsupported() {
    for source in ["print(1073741824)\n", "print(-1073741825)\n"] {
        assert_eq!(unsupported(source), "an int that does not fit in 31 bits");
    }
}

#[test]
fn the_floor_division_and_the_modulo_follow_python() {
    same_as_cpython(
        "the_floor_division_and_the_modulo_follow_python",
        "print(7 // 2, -7 // 2, 7 // -2, -7 // -2, 6 // 3, -6 // 3, 0 // -5)\n\
         print(7 % 3, -7 % 3, 7 % -3, -7 % -3, 6 % 3, -6 % -3, 0 % 5)\n\
         print(-1073741824 // 1, 1073741823 // -1, -1073741824 % -1, True // True)\n",
    );
}

#[test]
fn the_floor_division_of_the_least_int_by_minus_one_stops_the_program() {
    stops_at_the_int_limit(
        "the_floor_division_of_the_least_int_by_minus_one_stops_the_program",
        "print(-1073741824 // -1)\n",
        "",
        1,
        "    print(-1073741824 // -1)\n          ~~~~~~~~~~~~^^~~~\n",
        "1073741824",
    );
}

// The checker rejects a divisor that it knows to be 0, unless a comment
// silences the error.
#[test]
fn the_floor_division_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_floor_division_by_zero_raises_zero_division_error",
        "print(1)\nprint(7 // 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn the_modulo_by_zero_raises_zero_division_error() {
    same_as_cpython(
        "the_modulo_by_zero_raises_zero_division_error",
        "print(-7 % 0)  # ty: ignore[division-by-zero]\n",
    );
}

#[test]
fn the_true_division_is_unsupported() {
    assert_eq!(unsupported("print(7 / 2)\n"), "this operator");
}

#[test]
fn the_traceback_of_the_int_limit_shows_every_line_of_the_operator() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_shows_every_line_of_the_operator",
        "print('日本', (1073741823 +\n       1))\n",
        "",
        1,
        "    print('日本', (1073741823 +\n                   ~~~~~~~~~~~^\n           1))\n           ~\n",
        "1073741824",
    );
}

#[test]
fn the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line() {
    stops_at_the_int_limit(
        "the_traceback_of_the_int_limit_has_no_carets_under_a_whole_line",
        "-(-1073741823 - 1)\n",
        "",
        1,
        "    -(-1073741823 - 1)\n",
        "1073741824",
    );
}
