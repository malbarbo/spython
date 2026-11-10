//! The dataclasses.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

#[test]
fn a_dataclass_of_the_accepted_form_runs() {
    same_as_cpython(
        "a_dataclass_of_the_accepted_form_runs",
        indoc! {"
            import dataclasses
            from dataclasses import dataclass

            @dataclass
            class Tempo:
                \"\"\"Uma duração.\"\"\"

                horas: int
                minutos: 'int'

            @dataclass()
            class Vazia:
                pass

            @dataclasses.dataclass
            class Ponto:
                ...
                x: float
                y: float
                rotulo: str | None
                proximo: 'Ponto | None'
                tempo: Tempo

            def f(t: Tempo, p: Ponto | None) -> Ponto | None:
                return p

            print(1)
        "},
    );
}

/// The arguments run in the order of the call, and each one goes to the
/// field of its name.
#[test]
fn a_call_constructs_an_object_by_position_and_by_name() {
    same_as_cpython(
        "a_call_constructs_an_object_by_position_and_by_name",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class Tempo:
                horas: int
                minutos: int
                rotulo: str | None
                parte: float

            def f(x: int) -> int:
                print(x)
                return x

            t = Tempo(f(1), 2, 'a', 2.5)
            u = Tempo(f(2), rotulo=None, minutos=f(3), parte=1.0)
            v = Tempo(parte=0.5, rotulo='b', minutos=f(4), horas=f(5))
            w = u
            print(t is u, u is w, v is not None)
            if t:
                print('um objeto é verdadeiro')
        "},
    );
}

/// The call reads the name of the class, which has a value once the
/// `class` statement runs.
#[test]
fn a_call_before_the_class_raises_name_error() {
    same_as_cpython(
        "a_call_before_the_class_raises_name_error",
        indoc! {"
            from dataclasses import dataclass

            def f() -> None:
                if P(1):
                    print(1)

            def g() -> None:
                f()

            g()

            @dataclass
            class P:
                x: int
        "},
    );
}

#[test]
fn a_dataclass_outside_the_accepted_form_has_its_own_message() {
    for (source, what) in [
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                @dataclass
                class P:
                    x: int
            "},
            "a class with these decorators",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass(order=True)
                class P:
                    x: int
            "},
            "a dataclass with these arguments",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                def deco(c: type) -> type:
                    return c

                @deco
                class P:
                    x: int
            "},
            "this decorator of a class",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                @dataclass
                class Q(P):
                    y: int
            "},
            "a dataclass with a base",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                    def f(self) -> int:
                        return self.x
            "},
            "this statement in a dataclass",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int
                    print(1)
            "},
            "this statement in a dataclass",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    self: int
            "},
            "a field named self",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: list[int]
            "},
            "a field of this type",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: None
            "},
            "a field of this type",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                def f(a: P, b: P) -> bool:
                    return a == b
            "},
            "== of a dataclass",
        ),
        (
            indoc! {"
                from enum import Enum

                class Cor(Enum):
                    A = 1

                print(Cor(1) is Cor.A)
            "},
            "a call of this class",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
