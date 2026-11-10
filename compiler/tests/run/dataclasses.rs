//! The dataclasses.

use indoc::{formatdoc, indoc};

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

/// The `class` statement evaluates each default once, in the order of the
/// text, and a call without the argument takes the default.
#[test]
fn the_class_evaluates_the_defaults() {
    same_as_cpython(
        "the_class_evaluates_the_defaults",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum

            class Cor(Enum):
                A = 1

            def f(x: int) -> int:
                print('default', x)
                return x

            print('antes')

            @dataclass
            class P:
                \"\"\"Um ponto.\"\"\"

                a: int
                b: int = f(1)
                c: float = f(2) / 2
                d: str | None = None
                e: Cor = Cor.A

            @dataclass
            class Q:
                p: P | None = None

            print('depois')
            p = P(1)
            q = P(1, c=0.5)
            r = P(e=Cor.A, a=f(3))
            print(p is q)
        "},
    );
}

#[test]
fn a_default_raises_in_the_frame_of_the_class() {
    for (name, default) in [
        ("a_default_raises_in_the_frame_of_the_class", "f(0)"),
        ("a_default_raises_name_error_for_its_class", "h()"),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                from dataclasses import dataclass

                def f(x: int) -> int:
                    return 1 // x

                def h() -> int:
                    return 1 if P(1) else 2

                @dataclass
                class P:
                    \"\"\"Um ponto.

                    Com duas linhas.
                    \"\"\"

                    a: int
                    b: int = {default}
                    c: int = 2
            "},
        );
    }
}

/// The decorator rejects a default that is an object of a dataclass, after
/// the body of the class runs.
#[test]
fn a_default_of_a_dataclass_raises_value_error() {
    for (name, decorator) in [
        ("a_default_of_a_dataclass_raises_value_error", "@dataclass"),
        (
            "a_default_of_a_dataclass_raises_value_error_after_a_call",
            "@dataclass()",
        ),
        (
            "a_default_of_a_dataclass_raises_value_error_after_a_module",
            "@dataclasses.dataclass",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                import dataclasses
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                {decorator}
                class Q:
                    a: int = 1
                    p: P = P(1)
                    q: P = P(2)

                print(1)
            "},
        );
    }
}

/// CPython 3.13 evaluates an annotation without quotes in the body of the
/// class, after the default of the field.
#[test]
fn an_annotation_raises_name_error_for_a_class_without_a_value() {
    for (name, field) in [
        (
            "an_annotation_raises_name_error_for_its_own_class",
            "prox: No | None",
        ),
        (
            "an_annotation_raises_name_error_after_the_default",
            "prox: No | None = f()",
        ),
        (
            "an_annotation_raises_name_error_for_a_later_class",
            "segunda: Outra | None = None",
        ),
        ("an_annotation_in_quotes_runs", "prox: 'No | None' = f()"),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                from dataclasses import dataclass
                from enum import Enum

                class Cor(Enum):
                    A = 1

                def f() -> None:
                    print('default')
                    return None

                @dataclass
                class No:
                    v: int
                    cor: Cor | None
                    {field}

                @dataclass
                class Outra:
                    x: int

                print(1)
            "},
        );
    }
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
        (
            indoc! {"
                from dataclasses import dataclass

                x = 1

                @dataclass
                class P:
                    x: int = 2
                    y: int = x
            "},
            "a read of a name of the class in its body",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    outra: Outra | None = None

                @dataclass
                class Outra:
                    x: int
            "},
            "a name near a name of the class",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                def f() -> P | None:
                    return None

                @dataclass
                class Q:
                    p: P | None = f()
            "},
            "a default that may be an object of a dataclass",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
