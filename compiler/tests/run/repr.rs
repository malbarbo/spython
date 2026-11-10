//! `repr` of each type.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn repr_of_a_number_follows_python() {
    same_as_cpython(
        "repr_of_a_number_follows_python",
        indoc! {"
            n = -12
            x = 2.5
            inf = 1e308 * 10
            print(repr(0), repr(n), repr(1073741823), repr(True), repr(n < 0 and x < 0))
            print(repr(x), repr(1e16), repr(-0.0), repr(1 / 3), repr(5e-324), repr(inf), repr(inf - inf))
            print(repr(n) + '!' + repr(x), len(repr(1.0)), len(repr(n)), repr(x) == '2.5')
        "},
    );
}

#[test]
fn repr_of_none_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            def f() -> None:
                pass


            print(repr(f()))
        "}),
        "repr of this type"
    );
}

#[test]
fn repr_of_a_str_follows_python() {
    same_as_cpython(
        "repr_of_a_str_follows_python",
        indoc! {r#"
            print(repr(''), repr('abc'), repr("it's"), repr('say "hi"'), repr('\'"'), repr("''"))
            print(repr('a\tb\nc\rd\\e'), repr('\x00\x01\x1f\x7f'), repr('\x80\xa0\xad\xff'))
            print(repr('ação'), repr('\u0100\u2028\u200b\ufeff'), repr('\U0001f600\U000e0001\U0010ffff'))
            print(repr(' ~'), repr('\u3000\u00a0'), repr('日本語'), repr('\u0378'))
            s = 'a' + "'" * 2
            print(repr(s), len(repr(s)), repr(repr(s)), repr(s + '"'))
            print(repr('x' * 3) + repr(1) + repr(''))
        "#},
    );
}
