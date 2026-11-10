//! The operators of `str`, `len` and the index.

use indoc::{formatdoc, indoc};

use super::{path, run, same_as_cpython, unsupported};

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
            print('a b'.split())
        "}),
        "a call of this method"
    );
}

#[test]
fn find_and_index_look_for_a_str_in_a_slice() {
    same_as_cpython(
        "find_and_index_look_for_a_str_in_a_slice",
        indoc! {"
            s = 'abcabcé😀'
            print(s.find('a'), s.find('bc'), s.find('é😀'), s.find(''), s.find('x'))
            print(s.rfind('a'), s.rfind('bc'), s.rfind('é😀'), s.rfind(''), s.rfind('x'))
            print(s.find('a', 1), s.find('a', -5), s.find('a', 1, 3), s.find('a', 1, 4))
            print(s.rfind('a', 0, 3), s.rfind('a', -100), s.rfind('c', 0, -3), s.rfind('a', 4, 100))
            print(s.find('', 8), s.find('', 9), s.rfind('', 2, 5), s.find('a', None, 1))
            print(s.find('b', True), s.index('c'), s.rindex('c'), s.index('a', 2), s.rindex('b', 0, 4))
            print(''.find(''), ''.find('a'), ''.rfind(''), 'a'.find('aa'))
            print(s.index('z'))
        "},
    );
    same_as_cpython(
        "rindex_raises_valueerror",
        indoc! {"
            def last(s: str, sub: str) -> int:
                return s.rindex(sub)


            print(last('abc', 'c'))
            print(last('abc', 'd'))
        "},
    );
}

#[test]
fn count_gives_the_occurrences_that_do_not_overlap() {
    same_as_cpython(
        "count_gives_the_occurrences_that_do_not_overlap",
        indoc! {"
            s = 'aaaa😀a😀'
            print(s.count('a'), s.count('aa'), s.count('😀'), s.count('x'), s.count(''))
            print(s.count('a', 2), s.count('a', -3), s.count('aa', 1, 4), s.count('', 2, 5))
            print(s.count('', 7), s.count('', 8), s.count('', 9), s.count('', 5, 2), ''.count(''))
        "},
    );
}

#[test]
fn startswith_and_endswith_test_the_ends_of_a_slice() {
    same_as_cpython(
        "startswith_and_endswith_test_the_ends_of_a_slice",
        indoc! {"
            s = 'abcé😀'
            print(s.startswith('ab'), s.startswith('b'), s.startswith(''), s.startswith(s + 'x'))
            print(s.endswith('é😀'), s.endswith('é'), s.endswith(''), s.endswith(s))
            print(s.startswith('b', 1), s.startswith('c', -3), s.startswith('bc', 1, 2))
            print(s.endswith('b', 0, 2), s.endswith('c', 0, -2), s.endswith('', 5), s.endswith('', 6))
            print(s.startswith('', 5), s.startswith('', 6), ''.startswith(''), ''.endswith('a'))
        "},
    );
}

#[test]
fn in_tells_whether_a_str_is_in_another() {
    same_as_cpython(
        "in_tells_whether_a_str_is_in_another",
        indoc! {"
            def first(s: str) -> str:
                print('first', s)
                return s


            s = 'abcé😀'
            print('a' in s, 'cé' in s, '😀' in s, '' in s, 'x' in s, s + 'x' in s)
            print('a' not in s, 'x' not in s, '' in '', 'a' in '')
            print(first('a') in first('ba'))
        "},
    );
}

#[test]
fn replace_changes_each_occurrence_up_to_a_count() {
    same_as_cpython(
        "replace_changes_each_occurrence_up_to_a_count",
        indoc! {"
            s = 'aXbXXcé😀X'
            print(s.replace('X', '--'), s.replace('XX', '😀'), s.replace('X', ''), s.replace('z', 'y'))
            print(s.replace('X', '_', 2), s.replace('X', '_', 0), s.replace('X', '_', -1), s.replace('X', '_', 9))
            print('ab'.replace('', '-'), 'ab'.replace('', '-', 2), ''.replace('', 'x'), ''.replace('a', 'b'))
            print('aaa'.replace('aa', 'b'), 'abc'.replace('abc', ''), 'abc'.replace('', '', 3))
        "},
    );
}

#[test]
fn removeprefix_and_removesuffix_drop_an_end_that_matches() {
    same_as_cpython(
        "removeprefix_and_removesuffix_drop_an_end_that_matches",
        indoc! {"
            s = 'é😀abcé😀'
            print(s.removeprefix('é😀'), s.removeprefix('😀'), s.removeprefix(''), s.removeprefix(s))
            print(s.removesuffix('é😀'), s.removesuffix('é'), s.removesuffix(''), s.removesuffix(s))
            print(''.removeprefix(''), ''.removesuffix('a'), 'a'.removeprefix('ab'))
        "},
    );
}

#[test]
fn strip_drops_white_space_or_the_given_code_points_at_the_ends() {
    same_as_cpython(
        "strip_drops_white_space_or_the_given_code_points_at_the_ends",
        indoc! {"
            s = ' \\t\\n\\x0b\\x0c\\r\\x1c\\x1f\\x85\\xa0\\u1680\\u2000\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000a b\\u200b\\x1b '
            print(repr(s.strip()), repr(s.lstrip()), repr(s.rstrip()))
            t = 'xyé😀a😀yx'
            print(t.strip('xy'), t.lstrip('yx😀'), t.rstrip('😀xy'), t.strip(''), t.strip(None))
            print(repr(''.strip()), repr('   '.strip()), repr('   '.rstrip()), repr('xx'.strip('x')))
        "},
    );
}

#[test]
fn center_ljust_and_rjust_pad_a_str_to_a_width() {
    same_as_cpython(
        "center_ljust_and_rjust_pad_a_str_to_a_width",
        indoc! {"
            print(repr('a'.center(4, '*')), repr('ab'.center(5, '*')), repr('ab'.center(4)), repr('abc'.center(6)))
            print(repr('é😀'.center(5, '😀')), repr('abc'.center(2)), repr('abc'.center(-1)), repr(''.center(3)))
            print(repr('ab'.ljust(5, '.')), repr('ab'.ljust(1)), repr('ab'.rjust(5, '😀')), repr('ab'.rjust(2)))
            padded = 'x'.ljust(3, 'é')
            print(padded, 'x'.rjust(-3))
            print('a'.center(9, 'xy'))
        "},
    );
}

#[test]
fn zfill_pads_with_zeros_after_the_sign() {
    same_as_cpython(
        "zfill_pads_with_zeros_after_the_sign",
        indoc! {"
            print('42'.zfill(5), '-42'.zfill(5), '+42'.zfill(5), '42'.zfill(1), '-'.zfill(3), '+'.zfill(1))
            print(repr(''.zfill(3)), 'é😀'.zfill(4), '--1'.zfill(5), 'a-1'.zfill(5), '42'.zfill(-1))
        "},
    );
}

#[test]
fn expandtabs_moves_each_tab_to_the_next_column() {
    same_as_cpython(
        "expandtabs_moves_each_tab_to_the_next_column",
        indoc! {"
            s = 'a\\tbc\\tdéf\\t😀\\n\\tx\\r12\\ty'
            print(repr(s.expandtabs()), repr(s.expandtabs(4)), repr(s.expandtabs(1)))
            print(repr(s.expandtabs(0)), repr(s.expandtabs(-3)), repr(''.expandtabs()), repr('\\t'.expandtabs()))
        "},
    );
}

#[test]
fn a_slice_with_a_step_takes_every_kth_code_point() {
    same_as_cpython(
        "a_slice_with_a_step_takes_every_kth_code_point",
        indoc! {"
            def every(s: str, k: int) -> str:
                return s[::k]


            s = 'abcdeé😀'
            print(s[::2], s[::-1], s[1::3], s[::-2], s[5:1:-1], s[1:5:-1], s[-2::-3])
            print(s[-100:100:2], s[100:-100:-2], s[:None:1], s[None:None:None], s[2:2:5], s[::1073741823])
            print(repr(''[::-1]), s[::-1073741824], s[6:0:-2], s[-1:-8:-1], s[-1:-9:-1])
            print(every(s, 3))
            print(every(s, 0))
        "},
    );
}

#[test]
fn capitalize_title_and_swapcase_use_the_title_case_and_the_final_sigma() {
    same_as_cpython(
        "capitalize_title_and_swapcase_use_the_title_case_and_the_final_sigma",
        indoc! {"
            print('hello wORLD'.capitalize(), 'hello wORLD'.title(), 'hello wORLD'.swapcase())
            print('ǆemal ǅ'.capitalize(), 'ǆemal ǅ'.title(), 'ǆemal ǅ'.swapcase())
            print('ßtraße'.capitalize(), 'ßtraße'.title(), 'ßtraße'.swapcase())
            print('ﬁsh ﬂy'.capitalize(), 'ﬁsh ﬂy'.title(), 'ﬁsh ﬂy'.swapcase())
            print('ΣΑΣ σας'.capitalize(), 'ΣΑΣ σας'.title(), 'ΣΑΣ σας'.swapcase(), 'σΑΣ'.swapcase())
            print(\"they're o'neil\".title(), 'a1b2 3c'.title(), repr(''.title()), 'ŉ x'.capitalize())
            print('İstanbul'.swapcase(), 'ǰ'.title(), '𐐨𐐀'.swapcase(), 'Ⅻ ⓐ'.swapcase())
        "},
    );
}

#[test]
fn casefold_folds_each_code_point_without_the_final_sigma() {
    same_as_cpython(
        "casefold_folds_each_code_point_without_the_final_sigma",
        indoc! {"
            print('Straße ΟΔΟΣ ﬁ İ Ꭰꭰ ẞ 𐐀'.casefold(), repr(''.casefold()), 'ABC'.casefold() == 'abc')
        "},
    );
}

#[test]
fn the_predicates_of_a_str_test_every_code_point() {
    same_as_cpython(
        "the_predicates_of_a_str_test_every_code_point",
        indoc! {"
            def show(s: str) -> None:
                print(repr(s), s.isalpha(), s.isdecimal(), s.isdigit(), s.isnumeric(), s.isalnum(), s.isspace())


            show('')
            show('abcé')
            show('ab1')
            show('0123')
            show('٣४')
            show('²³')
            show('½Ⅻ')
            show('一二')
            show(' \\t\\u3000')
            show(' a')
            show('ǅ𐐀')
            show('𝟘𝟙')
        "},
    );
}

#[test]
fn isupper_islower_and_istitle_look_at_the_cased_code_points() {
    same_as_cpython(
        "isupper_islower_and_istitle_look_at_the_cased_code_points",
        indoc! {"
            def show(s: str) -> None:
                print(repr(s), s.isupper(), s.islower(), s.istitle())


            show('')
            show('ABC')
            show('abc')
            show('Abc')
            show('ABC1 !')
            show('abc1 !')
            show('123')
            show('Hello World')
            show('Hello world')
            show('HEllo')
            show('ǅungla')
            show('ǅ')
            show('Ǆ')
            show('ᾈ ᾐ')
            show('ß')
            show('Ⓐⓐ')
            show('ⓐ')
            show(\"They'Re\")
            show('A1B')
            show('𐐀𐐨')
        "},
    );
}

#[test]
fn isascii_isprintable_and_isidentifier_test_every_code_point() {
    same_as_cpython(
        "isascii_isprintable_and_isidentifier_test_every_code_point",
        indoc! {"
            def show(s: str) -> None:
                print(repr(s), s.isascii(), s.isprintable(), s.isidentifier())


            show('')
            show('abc')
            show('_x1')
            show('1x')
            show('é')
            show('a b')
            show('a\\n')
            show('\\x7f')
            show('ação')
            show('℘x')
            show('x·')
            show('𝔘𝔫𝔦')
            show('a-b')
            show('\\u00ad')
        "},
    );
}

#[test]
fn ord_gives_the_code_point_of_a_str_of_one() {
    same_as_cpython(
        "ord_gives_the_code_point_of_a_str_of_one",
        indoc! {"
            def code(s: str) -> int:
                return ord(s)


            print(ord('a'), ord('é'), ord('😀'), ord('\\x00'), code('\\U0010ffff'))
            print(code('ab'))
        "},
    );
    same_as_cpython(
        "ord_of_an_empty_str_raises_typeerror",
        indoc! {"
            print(ord(''))
        "},
    );
}

#[test]
fn chr_gives_the_str_of_a_code_point() {
    same_as_cpython(
        "chr_gives_the_str_of_a_code_point",
        indoc! {"
            def char(i: int) -> str:
                return chr(i)


            print(chr(97), chr(233), chr(128512), repr(chr(0)), char(1114111) == '\\U0010ffff', chr(True))
            print(char(55295), char(57344))
            print(char(1114112))
        "},
    );
    same_as_cpython(
        "chr_of_a_negative_int_raises_valueerror",
        indoc! {"
            print(chr(-1))
        "},
    );
}

#[test]
fn chr_of_a_surrogate_stops_the_program() {
    let name = "chr_of_a_surrogate_stops_the_program";
    let output = run(
        name,
        indoc! {"
            def char(i: int) -> str:
                return chr(i)


            print(char(55296))
        "},
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        formatdoc! {"
            Traceback (most recent call last):
              File \"{file}\", line 5, in <module>
                print(char(55296))
                      ~~~~^^^^^^^
              File \"{file}\", line 2, in char
                return chr(i)
            spython: chr(55296) gives a surrogate, which a str of spython does not hold
        ", file = path(name, "py").display()}
    );
    assert_eq!(output.status.code(), Some(1));
}
