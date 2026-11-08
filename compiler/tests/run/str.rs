//! The operators of `str`, `len` and the index.

use indoc::{formatdoc, indoc};

use super::{same_as_cpython, unsupported};

#[test]
fn the_product_of_a_str_and_an_int_repeats_the_str() {
    same_as_cpython(
        "the_product_of_a_str_and_an_int_repeats_the_str",
        indoc! {"
            n = 3
            print('ab' * n, n * 'é', 'ab' * 0, 'ab' * -2, '' * 5, 'x' * True, False * 'x')
            s = 'ab'
            s *= 5
            print(s)
            print('xyz' * 7)
        "},
    );
}

#[test]
fn the_product_of_an_int_and_a_str_evaluates_the_int_first() {
    same_as_cpython(
        "the_product_of_an_int_and_a_str_evaluates_the_int_first",
        indoc! {"
            def f(s: str) -> str:
                print(s)
                return s


            def g(n: int) -> int:
                print(n)
                return n


            print(g(2) * f('a'))
            print(f('b') * g(3))
        "},
    );
}

#[test]
fn len_gives_the_number_of_code_points_of_a_str() {
    same_as_cpython(
        "len_gives_the_number_of_code_points_of_a_str",
        indoc! {"
            s = 'aé😀'
            print(len(s), len(''), len(s * 100) + 1, len('ab' + s) == 5)
        "},
    );
}

#[test]
fn len_of_a_bool_or_an_int_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print(len(1))  # ty: ignore[invalid-argument-type]
        "}),
        "len of this type"
    );
}

#[test]
fn an_index_gives_the_code_point_of_a_str() {
    same_as_cpython(
        "an_index_gives_the_code_point_of_a_str",
        indoc! {"
            s = 'aé😀z'
            n = 2
            print(s[0], s[1], s[n], s[3], s[-1], s[-4], s[True], s[0][0][-1])
            print(s[0] + s[0] == 'aa', s[2] * 2, len(s[2]))
        "},
    );
}

#[test]
fn an_index_evaluates_the_str_first() {
    same_as_cpython(
        "an_index_evaluates_the_str_first",
        indoc! {"
            def f(s: str) -> str:
                print(s)
                return s


            def g(n: int) -> int:
                print(n)
                return n


            print(f('ab')[g(1)])
        "},
    );
}

#[test]
fn an_index_out_of_range_raises_index_error() {
    same_as_cpython(
        "an_index_out_of_range_raises_index_error",
        indoc! {"
            def at(s: str, n: int) -> str:
                return s[n]


            print(at('abc', -3))
            print(at('abc', 3))
        "},
    );
    same_as_cpython(
        "an_index_out_of_range_raises_index_error_from_the_end",
        indoc! {"
            def at(s: str, n: int) -> str:
                return s[n]


            print(at('abc', -4))
        "},
    );
    same_as_cpython(
        "an_index_of_the_empty_str_raises_index_error",
        indoc! {"
            def at(s: str, n: int) -> str:
                return s[n]


            print(at('', 0))
        "},
    );
}

#[test]
fn a_slice_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
        print('abc'[1:2])
    "}),
        "a slice"
    );
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
            source.push_str(&formatdoc! {"
                print('{a}' == '{b}', '{a}' != '{b}', '{a}' < '{b}', '{a}' <= '{b}', '{a}' > '{b}', '{a}' >= '{b}')
            "});
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
        indoc! {"
            def f() -> str:
                print('f')
                return 'z'


            a = 'a'
            print(a < 'b' < 'c', a < 'c' < 'b', 'b' < a < f(), a < 'b' < f(), a == a != 'b')
        "},
    );
}

#[test]
fn a_comparison_of_a_str_and_an_int_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print('a' == 1)
        "}),
        "a comparison of two types"
    );
}
