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
fn a_slice_gives_the_code_points_between_two_indices() {
    same_as_cpython(
        "a_slice_gives_the_code_points_between_two_indices",
        indoc! {"
            s = 'ação é'
            n = len(s)
            print(s[1:3], s[:2], s[2:], s[:], s[0:n], s[1:-1], s[-3:], s[:-2], s[-2:-1])
            print(s[3:1] + '|', s[10:] + '|', s[:10], s[-10:2], s[5:-10] + '|', s[n:] + '|')
            print(s[True:], s[:False] + '|', s[None:2], s[1:None], len(s[2:5]), s[2:5][1])
            print(''[1:] + '|', ''[:-1] + '|', 'a'[0:1], s[1073741823:] + '|', s[-1073741824:])
        "},
    );
}

#[test]
fn a_slice_evaluates_the_str_and_then_each_index() {
    same_as_cpython(
        "a_slice_evaluates_the_str_and_then_each_index",
        indoc! {"
            def text(s: str) -> str:
                print('text', s)
                return s


            def index(n: int) -> int:
                print('index', n)
                return 6 // n


            print(text('abcdef')[index(3):index(2)])
            print(text('abcdef')[index(0):index(1)])
        "},
    );
}

#[test]
fn a_slice_with_a_step_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
        print('abc'[::2])
    "}),
        "a slice with a step"
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

#[test]
fn str_of_a_number_follows_python() {
    same_as_cpython(
        "str_of_a_number_follows_python",
        indoc! {"
            n = -12
            x = 2.5
            inf = 1e308 * 10
            print(str(), str(0), str(n), str(1073741823), str(True), str(n < 0 and x < 0))
            print(str(x), str(1e16), str(-0.0), str(1 / 3), str(5e-324), str(inf), str(inf - inf))
            print(str('ab'), str(n) + '!' + str(x), len(str(1.0)), len(str(n)), str(str(7)))
            print(str(x) == '2.5', str(n) < str(x), str(2.50) * 2)
        "},
    );
}

#[test]
fn an_f_string_joins_the_str_of_each_field() {
    same_as_cpython(
        "an_f_string_joins_the_str_of_each_field",
        indoc! {r#"
            n = 7
            x = 0.1
            s = 'é'
            print(f"{n}", f"{x}", f"{s}", f"{n > 3}", len(f"{x}{n}"))
            print(f"n={n}, x={ x + 0.2 }, s={s!s}, b={not n}{{}}")
            print('a' f"{n}" "b" f"c{s}", f'{f"{n}{x}"}!', f"{'q'}{1}{2.0}")
            print(f"{n}" + f"{n}" * 2, f"{s}" == s, f"""{n}
            {x}""")
        "#},
    );
}

#[test]
fn an_f_string_evaluates_every_field_from_left_to_right() {
    same_as_cpython(
        "an_f_string_evaluates_every_field_from_left_to_right",
        indoc! {r#"
            def f(n: int) -> int:
                print('f', n)
                return n


            print(f"{f(1)} and {f(2)}{f(3) // f(0)}")
        "#},
    );
}

#[test]
fn a_field_with_a_conversion_or_a_format_is_unsupported() {
    for field in ["{x!r}", "{x!a}", "{x:.2f}", "{x:}", "{x=}"] {
        assert_eq!(
            unsupported(&format!("x = 2.5\nprint(f\"{field}\")\n")),
            "a replacement field with a conversion or a format",
            "{field}"
        );
    }
}

#[test]
fn str_of_none_is_unsupported() {
    let source = indoc! {"
        def f() -> None:
            pass


        print(str(f()))
    "};
    assert_eq!(unsupported(source), "str of None");
    assert_eq!(
        unsupported(&source.replace("str(f())", "f'{f()}'")),
        "str of None"
    );
}

#[test]
fn upper_and_lower_map_each_code_point_as_unicode_says() {
    same_as_cpython(
        "upper_and_lower_map_each_code_point_as_unicode_says",
        indoc! {"
            s = 'Olá, Mundo! ß ﬃ İ ŉ ǅ 𐐨𐐀 ΐ 123'
            print(s.upper())
            print(s.lower())
            print(''.upper(), ''.lower(), 'abc'.upper().lower())
            print(len('ß'.upper()), len('İ'.lower()), len('ΐ'.upper()))
        "},
    );
}

#[test]
fn lower_gives_the_final_sigma_at_the_end_of_a_word() {
    same_as_cpython(
        "lower_gives_the_final_sigma_at_the_end_of_a_word",
        indoc! {"
            print('ΟΔΟΣ ΟΔΟΣ.'.lower())
            print('Σ'.lower(), 'ΑΣ'.lower(), 'ΣΑ'.lower(), 'ΑΣΑ'.lower())
            print(\"Α'Σ\".lower(), \"ΑΣ'\".lower(), \"ΑΣ'Α\".lower(), \"ΑΣ'.\".lower())
            print('1Σ'.lower(), 'Α\u{301}Σ'.lower(), 'ΑΣ\u{301}Β'.lower())
        "},
    );
}

#[test]
fn a_method_of_a_str_that_the_prelude_lacks_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            print('a'.title())
        "}),
        "a call of this method"
    );
}
