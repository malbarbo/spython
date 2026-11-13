//! `repr` of each type.

use std::path::Path;

use indoc::{formatdoc, indoc};

use super::{compile, same_as_cpython, unsupported_at};

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
fn repr_of_a_union_tests_the_type_of_the_value() {
    same_as_cpython(
        "repr_of_a_union_tests_the_type_of_the_value",
        indoc! {r#"
            from dataclasses import dataclass
            from enum import Enum


            class Cor(Enum):
                AZUL = 'azul'


            @dataclass
            class P:
                x: int | str
                s: str | None
                q: 'P | Cor | None'


            def f() -> None:
                pass


            def pick(i: int) -> int | float | bool | str | None:
                if i == 0:
                    return 1
                if i == 1:
                    return 2.5
                if i == 2:
                    return True
                if i == 3:
                    return "it's"
                return None


            print(repr(f()), repr(pick(0)), repr(pick(1)), repr(pick(2)), repr(pick(3)), repr(pick(4)))
            s: str | None = 'a'
            print(repr(s), f'{s!r}', f'{pick(3)!r}')
            s = None
            print(repr(s), f'{s!r}')
            p = P('x', None, P(1, 'b', Cor.AZUL))
            print(p, repr(p))
        "#},
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

#[test]
fn repr_of_a_str_or_none_leaves_out_the_repr_of_the_classes() {
    let program = |t: &str| {
        formatdoc! {r#"
            from dataclasses import dataclass


            @dataclass
            class P:
                x: int
                y: float
                nome: str


            @dataclass
            class Q:
                p: P
                n: int


            def f(s: {t}) -> str:
                return repr(s)


            p = Q(P(1, 2.5, 'a'), 3)
            print(f('a'), p.n)
        "#}
    };
    let size = |t| {
        compile(&program(t), Path::new("size.py"))
            .expect("the program to compile")
            .bytes
            .len()
    };
    let (str, or_none) = (size("str"), size("str | None"));
    assert!(or_none < str + 200, "{str} and {or_none}");
}

#[test]
fn the_repr_of_a_function_is_unsupported_where_the_code_asks_for_it() {
    let source = indoc! {"
        from dataclasses import dataclass
        from typing import Callable


        @dataclass
        class Passo:
            f: Callable[[int], int]


        def dobro(x: int) -> int:
            return 2 * x


        print(PLACE)
    "};
    for (place, code) in [
        ("repr(dobro)", "repr(dobro)"),
        ("Passo(dobro)", "f: Callable[[int], int]"),
    ] {
        assert_eq!(
            unsupported_at(&source.replace("PLACE", place)),
            ("the repr of a function".to_owned(), code)
        );
    }
}
