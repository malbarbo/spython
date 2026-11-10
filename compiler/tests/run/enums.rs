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

/// CPython evaluates the annotations of a `def` when the `def` runs, after
/// the defaults, and an annotation in quotes stays a `str`.
#[test]
fn a_def_raises_name_error_for_a_class_in_an_annotation() {
    for (name, def) in [
        (
            "a_def_raises_name_error_for_a_class_in_an_annotation",
            "def f(c: int | Cor = 1, d: 'Cor | None' = g()) -> 'Cor | None':",
        ),
        (
            "a_def_raises_name_error_for_a_class_in_its_result",
            "def f(c: 'Cor | None' = g()) -> Cor | None:",
        ),
        (
            "a_def_runs_with_a_class_in_quotes",
            "def f(c: 'Cor | None' = g()) -> 'Cor | None':",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                from enum import Enum

                def g() -> None:
                    print('g')

                {def}
                    return None

                print(f() is None)

                class Cor(Enum):
                    A = 1
            "},
        );
    }
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
            "a class other than an Enum",
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
