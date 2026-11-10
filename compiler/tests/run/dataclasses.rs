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
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
