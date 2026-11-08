//! The operators of `str`, `len` and the index.

use super::{same_as_cpython, unsupported};

#[test]
fn the_product_of_a_str_and_an_int_repeats_the_str() {
    same_as_cpython(
        "the_product_of_a_str_and_an_int_repeats_the_str",
        "n = 3\nprint('ab' * n, n * 'é', 'ab' * 0, 'ab' * -2, '' * 5, 'x' * True, False * 'x')\ns = 'ab'\ns *= 5\nprint(s)\nprint('xyz' * 7)\n",
    );
}

#[test]
fn the_product_of_an_int_and_a_str_evaluates_the_int_first() {
    same_as_cpython(
        "the_product_of_an_int_and_a_str_evaluates_the_int_first",
        "def f(s: str) -> str:\n    print(s)\n    return s\n\n\ndef g(n: int) -> int:\n    print(n)\n    return n\n\n\nprint(g(2) * f('a'))\nprint(f('b') * g(3))\n",
    );
}

#[test]
fn len_gives_the_number_of_code_points_of_a_str() {
    same_as_cpython(
        "len_gives_the_number_of_code_points_of_a_str",
        "s = 'aé\u{1f600}'\nprint(len(s), len(''), len(s * 100) + 1, len('ab' + s) == 5)\n",
    );
}

#[test]
fn len_of_a_bool_or_an_int_is_unsupported() {
    assert_eq!(
        unsupported("print(len(1))  # ty: ignore[invalid-argument-type]\n"),
        "len of this type"
    );
}

#[test]
fn an_index_gives_the_code_point_of_a_str() {
    same_as_cpython(
        "an_index_gives_the_code_point_of_a_str",
        "s = 'aé\u{1f600}z'\nn = 2\nprint(s[0], s[1], s[n], s[3], s[-1], s[-4], s[True], s[0][0][-1])\nprint(s[0] + s[0] == 'aa', s[2] * 2, len(s[2]))\n",
    );
}

#[test]
fn an_index_evaluates_the_str_first() {
    same_as_cpython(
        "an_index_evaluates_the_str_first",
        "def f(s: str) -> str:\n    print(s)\n    return s\n\n\ndef g(n: int) -> int:\n    print(n)\n    return n\n\n\nprint(f('ab')[g(1)])\n",
    );
}

#[test]
fn an_index_out_of_range_raises_index_error() {
    same_as_cpython(
        "an_index_out_of_range_raises_index_error",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('abc', -3))\nprint(at('abc', 3))\n",
    );
    same_as_cpython(
        "an_index_out_of_range_raises_index_error_from_the_end",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('abc', -4))\n",
    );
    same_as_cpython(
        "an_index_of_the_empty_str_raises_index_error",
        "def at(s: str, n: int) -> str:\n    return s[n]\n\n\nprint(at('', 0))\n",
    );
}

#[test]
fn a_slice_is_unsupported() {
    assert_eq!(unsupported("print('abc'[1:2])\n"), "a slice");
}

#[test]
fn the_comparisons_of_str_follow_the_order_of_the_code_points() {
    let strs = [
        "",
        "a",
        "b",
        "ab",
        "aa",
        "B",
        "é",
        "\u{1f600}",
        "a\u{1f600}",
    ];
    let mut source = String::new();
    for a in strs {
        for b in strs {
            source.push_str(&format!(
                "print('{a}' == '{b}', '{a}' != '{b}', '{a}' < '{b}', '{a}' <= '{b}', '{a}' > '{b}', '{a}' >= '{b}')\n"
            ));
        }
    }
    same_as_cpython(
        "the_comparisons_of_str_follow_the_order_of_the_code_points",
        &source,
    );
}

#[test]
fn a_chain_of_comparisons_of_str_stops_at_the_first_false() {
    same_as_cpython(
        "a_chain_of_comparisons_of_str_stops_at_the_first_false",
        "def f() -> str:\n    print('f')\n    return 'z'\n\n\na = 'a'\nprint(a < 'b' < 'c', a < 'c' < 'b', 'b' < a < f(), a < 'b' < f(), a == a != 'b')\n",
    );
}

#[test]
fn a_comparison_of_a_str_and_an_int_is_unsupported() {
    assert_eq!(
        unsupported("print('a' == 1)\n"),
        "a comparison of two types"
    );
}
