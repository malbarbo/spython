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
