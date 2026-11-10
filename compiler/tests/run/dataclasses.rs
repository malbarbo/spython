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
fn a_program_reads_and_assigns_the_fields() {
    same_as_cpython(
        "a_program_reads_and_assigns_the_fields",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum

            class Cor(Enum):
                A = 1
                B = 2

            @dataclass
            class No:
                v: int
                cor: Cor
                nome: str | None
                parte: float = 0.5
                prox: 'No | None' = None

            def soma(n: No | None) -> int:
                total = 0
                while n is not None:
                    total += n.v
                    n = n.prox
                return total

            def obj(n: No) -> No:
                print('objeto')
                return n

            def val(x: int) -> int:
                print('valor')
                return x

            a = No(1, Cor.A, None)
            b = No(2, Cor.B, 'b', prox=a)
            print(b.v, b.prox is a, b.cor == Cor.B, b.cor.name, b.nome, b.parte)
            b.v = 10
            b.v += 5
            b.parte *= 3
            b.nome = None
            print(b.v, b.nome is None, b.parte)
            if b.prox is not None:
                print(b.prox.v, b.prox.cor.value)
            print(soma(b))
            a.prox = b
            print(a.prox is b, a.prox.prox is a)
            obj(a).v = val(7)
            print(a.v)
        "},
    );
}

#[test]
fn the_repr_of_a_dataclass_has_the_repr_of_each_field() {
    same_as_cpython(
        "the_repr_of_a_dataclass_has_the_repr_of_each_field",
        indoc! {r#"
            from dataclasses import dataclass
            from enum import Enum

            class Cor(Enum):
                A = 1

            @dataclass
            class Vazia:
                pass

            @dataclass
            class No:
                v: int
                parte: float
                ok: bool
                nome: str
                apelido: str | None
                cor: Cor
                talvez: Cor | None
                vazia: Vazia
                prox: 'No | None' = None

            a = No(1, 0.5, True, "d'água", None, Cor.A, None, Vazia())
            b = No(-2, 1e300, False, 'b', 'x', Cor.A, Cor.A, Vazia(), a)
            print(a)
            print(repr(b))
            print(str(b) == repr(b), f'<{a}>')
            print(Vazia())
        "#},
    );
}

/// Each level of the `repr` has two frames, which CPython does not join,
/// and the recursion reaches the limit at half the depth of a function.
#[test]
fn a_deep_repr_raises_recursion_error() {
    same_as_cpython(
        "a_deep_repr_raises_recursion_error",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int
                prox: 'No | None'

            def lista(n: int) -> No | None:
                if n == 0:
                    return None
                return No(n, lista(n - 1))

            print(repr(lista(400))[:30])
            print(str(lista(0)), f'{lista(1)}')
            print(lista(600))
        "},
    );
}

/// The `repr` of an object inside its own `repr` is `...`.
#[test]
fn the_repr_of_a_cycle_writes_three_dots() {
    same_as_cpython(
        "the_repr_of_a_cycle_writes_three_dots",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int
                prox: 'No | None'

            @dataclass
            class Par:
                a: No
                b: No

            a = No(1, None)
            a.prox = a
            print(a)
            b = No(2, a)
            a.prox = b
            print(b, a)
            print(Par(a, a))
            print(repr(a) == str(a))
        "},
    );
}

/// The `__eq__` of a dataclass returns `True` for the same object before
/// it compares the fields, so an object with a `nan` or with a cycle is
/// equal to itself.
#[test]
fn objects_of_a_dataclass_compare_by_their_fields() {
    same_as_cpython(
        "objects_of_a_dataclass_compare_by_their_fields",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum

            class Cor(Enum):
                A = 1
                B = 2

            @dataclass
            class P:
                x: int
                y: float
                nome: str | None
                cor: Cor | None

            @dataclass
            class Q:
                x: int

            @dataclass
            class No:
                v: int
                prox: 'No | None'

            @dataclass
            class Vazia:
                pass

            def iguais(p: P | None, q: P | None) -> bool:
                return p == q

            a = P(1, 2.0, None, Cor.A)
            b = P(1, 2.0, None, Cor.A)
            c = P(1, 2.0, 'c', None)
            print(a == b, a != b, a == c, a != c, a == a)
            n = P(1, float('nan'), None, None)
            print(n == n, n == P(1, n.y, None, None))
            print(iguais(None, None), iguais(a, None), iguais(None, a), iguais(a, b))
            print(a == Q(1), Q(1) != a, Q(1) == 1, Vazia() == Vazia())
            l1 = No(1, No(2, None))
            print(l1 == No(1, No(2, None)), l1 == No(1, None), l1 != No(1, No(3, None)))
            c1 = No(1, None)
            c1.prox = c1
            print(c1 == c1, c1 != c1)
            c2 = No(1, None)
            c2.prox = c2
            print(c1 == c2)
        "},
    );
}

/// A frozen dataclass is hashable, so an object of it may be a default.
#[test]
fn a_frozen_dataclass_runs() {
    same_as_cpython(
        "a_frozen_dataclass_runs",
        indoc! {"
            import dataclasses
            from dataclasses import dataclass

            @dataclass(frozen=True)
            class Ponto:
                x: int
                y: int = 0

            @dataclasses.dataclass(frozen=True)
            class No:
                v: int
                prox: 'No | None'

            @dataclass
            class Caixa:
                canto: Ponto = Ponto(1, 2)

            def lista(n: int) -> No | None:
                if n == 0:
                    return None
                return No(n, lista(n - 1))

            p = Ponto(3)
            print(p, p.x + p.y, p == Ponto(3, 0), Caixa(), Caixa().canto is Caixa().canto)
            print(lista(2), lista(3) == lista(3), lista(3) != lista(2))
            print(lista(600))
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

                @dataclass(frozen=False)
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

                def f(a: P, b: P, c: P) -> bool:
                    return a == b == c
            "},
            "a chain of comparisons of dataclasses",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                def f(a: P | int, b: P | int) -> bool:
                    return a == b
            "},
            "== of a union with a dataclass",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int

                @dataclass
                class Q:
                    p: P | int

                def f(a: Q, b: Q) -> bool:
                    return a == b
            "},
            "== of a field of a union with a dataclass",
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

                @dataclass
                class P:
                    x: int

                def f() -> P:
                    return P(1)

                f().x += 1
            "},
            "this target",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class P:
                    x: int | str

                print(P(1))
            "},
            "the repr of a field of a union",
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
