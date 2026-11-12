//! The classes of `Enum`.

use indoc::{formatdoc, indoc};

use super::{same_as_cpython, unsupported};

#[test]
fn an_enum_of_the_accepted_form_runs() {
    same_as_cpython(
        "an_enum_of_the_accepted_form_runs",
        indoc! {"
            import enum
            from enum import Enum, auto

            class Cor(Enum):
                \"\"\"As cores de um semáforo.\"\"\"

                VERDE = auto()
                AMARELO = -2
                VERMELHO = auto()

            class Vazio(enum.Enum):
                pass

            class Nome(Enum):
                ...
                A = 'a'
                B = 'b'

            class Seguinte(Enum):
                X = enum.auto()
                Y = enum.auto()

            print(1)
        "},
    );
}

/// A member is a reference to a struct, which may be null for `None`.
#[test]
fn a_function_takes_and_gives_members() {
    same_as_cpython(
        "a_function_takes_and_gives_members",
        indoc! {"
            from enum import Enum

            class Cor(Enum):
                A = 1

            def f(c: Cor, d: Cor | None) -> Cor | None:
                if d is None:
                    return c
                if c and d:
                    return d
                return None

            print(1)
        "},
    );
}

/// CPython 3.13 evaluates an annotation without quotes of a `def` and of an
/// assignment of the module when the statement runs, and 3.14 does not.
#[test]
fn an_annotation_of_the_module_with_a_later_class_does_not_compile() {
    for statement in [
        "def f(c: int | Cor = 1) -> None:\n    return None",
        "def f(c: 'Cor | None' = None) -> Cor | None:\n    return None",
        "x: Cor | None = None",
        "y: Cor",
    ] {
        let what = unsupported(&formatdoc! {"
            from enum import Enum

            {statement}

            class Cor(Enum):
                A = 1
        "});
        assert_eq!(
            what, "a class in an annotation before its `class` statement",
            "{statement}"
        );
    }
}

/// A function does not evaluate the annotation of an assignment, and an
/// annotation in quotes stays a `str`.
#[test]
fn an_annotation_that_does_not_run_takes_a_later_class() {
    same_as_cpython(
        "an_annotation_that_does_not_run_takes_a_later_class",
        indoc! {"
            from enum import Enum

            def f(c: 'Cor | None' = None) -> 'Cor | None':
                z: Cor | None = c
                return z

            x: 'Cor | None' = f()
            print(x is None)

            class Cor(Enum):
                A = 1

            print(f(Cor.A))
        "},
    );
}

/// Each member is one object, which `_start` makes before the program runs.
#[test]
fn a_member_is_a_global_of_the_module() {
    same_as_cpython(
        "a_member_is_a_global_of_the_module",
        indoc! {"
            from enum import Enum

            class Cor(Enum):
                A = 1
                B = 2

            class Nome(Enum):
                X = 'x'

            def f(c: Cor) -> Cor | None:
                if c:
                    return c
                return None

            x = Cor.A
            y: Nome | None = None
            print(f(x) is None, y is None, f(Cor.B) is not None, Nome.X is None)
        "},
    );
}

#[test]
fn a_member_has_a_name_and_a_value() {
    same_as_cpython(
        "a_member_has_a_name_and_a_value",
        indoc! {"
            from enum import Enum, auto

            class Cor(Enum):
                VERDE = auto()
                AMARELO = 5

            class Nome(Enum):
                X = 'xis'

            def nome(c: Cor) -> str:
                return c.name

            c = Cor.AMARELO
            print(nome(Cor.VERDE), Cor.VERDE.value, c.name, c.value + 1)
            print(Nome.X.value.upper(), Nome.X.name)
        "},
    );
}

/// A member is equal to itself alone. The checker proves the chain of the
/// `if` complete, so the variable has a value after it.
#[test]
fn a_member_is_equal_to_itself_alone() {
    same_as_cpython(
        "a_member_is_equal_to_itself_alone",
        indoc! {"
            from enum import Enum, auto

            class Sinal(Enum):
                VERDE = auto()
                AMARELO = auto()
                VERMELHO = auto()

            def proximo(s: Sinal) -> Sinal:
                if s == Sinal.VERDE:
                    p = Sinal.AMARELO
                elif s == Sinal.AMARELO:
                    p = Sinal.VERMELHO
                elif s == Sinal.VERMELHO:
                    p = Sinal.VERDE
                return p

            s = Sinal.VERDE
            print(proximo(s).name, proximo(proximo(s)).name, proximo(s) != s)
            x: Sinal | None = None
            print(x != Sinal.VERDE, s is Sinal.VERDE)
            print(s is not proximo(s), x is s, x is not None)
            print(Sinal.VERDE == Sinal.VERDE == s, s == s != proximo(s))
        "},
    );
}

/// `str` writes the class and the name of a member, and `repr` writes the
/// `repr` of the value too.
#[test]
fn a_member_has_a_str_and_a_repr() {
    same_as_cpython(
        "a_member_has_a_str_and_a_repr",
        indoc! {r#"
            from enum import Enum, auto

            class Cor(Enum):
                VERDE = auto()
                AMARELO = -3

            class Nome(Enum):
                X = "it's"
                Y = 'a\nb'

            def mostra(n: Nome | None) -> None:
                print(n)

            c = Cor.VERDE
            print(c, Cor.AMARELO, Nome.X)
            print(str(c), repr(c), repr(Cor.AMARELO), repr(Nome.X), repr(Nome.Y))
            print(f'{c} {c!r} {Nome.Y!r}')
            mostra(None)
            mostra(Nome.Y)
        "#},
    );
}

/// A uniform value holds an object, and the tests of its type find the
/// class. An object is equal to itself alone, so a member is not equal to
/// its value.
#[test]
fn a_uniform_value_holds_a_member() {
    same_as_cpython(
        "a_uniform_value_holds_a_member",
        indoc! {"
            from enum import Enum, auto

            class Cor(Enum):
                A = auto()
                B = auto()

            class Nome(Enum):
                X = 'x'

            def valor(n: int) -> int | Cor:
                if n > 0:
                    return n
                return Cor.B

            def nome(n: int) -> Nome | Cor | None:
                if n > 1:
                    return Nome.X
                if n > 0:
                    return Cor.A
                return None

            v = valor(0)
            print(v, valor(3), nome(2), nome(1), nome(0))
            print(v == Cor.B, v == 2, Cor.A == 1, Cor.A != 1, valor(1) == 1)
            print(Cor.A == Nome.X, nome(2) == Nome.X, nome(1) != Nome.X)
            print(Cor.A if v == Cor.B else 0, 1 if v == 0 else Nome.X)
        "},
    );
}

/// A read of a member reads the class, which raises `NameError` before the
/// `class` statement.
#[test]
fn a_member_before_its_class_raises_name_error() {
    same_as_cpython(
        "a_member_before_its_class_raises_name_error",
        indoc! {"
            from enum import Enum

            def f() -> 'Cor':
                return Cor.A

            print(f() is None)

            class Cor(Enum):
                A = 1
        "},
    );
}

/// The name of a class is a global, which a `NameError` suggests once the
/// `class` statement has run.
#[test]
fn the_name_error_suggests_a_class_after_its_statement() {
    for (name, before, after) in [
        (
            "the_name_error_suggests_a_class_after_its_statement",
            "class Cores(Enum):\n    A = 1\n",
            "",
        ),
        (
            "the_name_error_suggests_no_class_before_its_statement",
            "",
            "class Cores(Enum):\n    A = 1\n",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                from enum import Enum

                {before}
                def f() -> int:
                    return Core

                print(f())
                Core = 1
                {after}"},
        );
    }
}

#[test]
fn an_enum_outside_the_accepted_form_has_its_own_message() {
    for (source, what) in [
        (
            indoc! {"
                class P:
                    x: int = 1
            "},
            "this statement in a class",
        ),
        (
            indoc! {"
                from enum import IntEnum as E
            "},
            "an import of this name",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1
                    B: int = 2
            "},
            "this statement in an Enum",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1.5
            "},
            "a member of an Enum with this value",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1
                    B = 'b'
            "},
            "an Enum with values of two types",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1
                    B = 1
            "},
            "two members of an Enum with the same value",
        ),
        (
            indoc! {"
                from enum import Enum, auto

                class Cor(Enum):
                    X = 5
                    Y = auto()
            "},
            "an Enum whose values the checker computes otherwise",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1073741824
            "},
            "an int that does not fit in 31 bits",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1

                x = Cor.__name__
            "},
            "this attribute of a class",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1

                x = Cor
            "},
            "a class as a value",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1

                x = Cor.A._name_
            "},
            "this attribute of an object",
        ),
        (
            indoc! {"
                from enum import Enum

                def f() -> None:
                    class Cor(Enum):
                        A = 1
            "},
            "a class in a function",
        ),
        (
            indoc! {"
                from enum import Enum

                if 1 > 0:
                    class Cor(Enum):
                        A = 1
            "},
            "a class in an if or a while",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

#[test]
fn an_enum_calls_its_methods() {
    same_as_cpython(
        "an_enum_calls_its_methods",
        indoc! {"
            from enum import Enum, auto

            def f(x: int) -> int:
                print('default', x)
                return 10 // x

            class Cor(Enum):
                VERMELHO = auto()
                VERDE = auto()
                AZUL = auto()

                def proxima(self) -> 'Cor':
                    if self == Cor.VERMELHO:
                        return Cor.VERDE
                    elif self == Cor.VERDE:
                        return Cor.AZUL
                    return Cor.VERMELHO

                def peso(self, fator: int = f(2)) -> int:
                    return self.value * fator

                def nome(self) -> str:
                    return self.name.lower()

            c = Cor.AZUL.proxima()
            print(c, c.proxima().nome(), c.peso(), Cor.AZUL.peso(fator=3))

            class Ruim(Enum):
                A = 1

                def m(self, x: int = f(0)) -> int:
                    return x
        "},
    );
}

#[test]
fn a_special_method_in_an_enum_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            from enum import Enum

            class Cor(Enum):
                A = 1

                def __eq__(self, other: object) -> bool:
                    return True
        "}),
        "a special method in an Enum"
    );
}
