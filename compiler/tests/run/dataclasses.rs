//! The dataclasses.

use indoc::{formatdoc, indoc};

use super::{run, same_as_cpython, unsupported};

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
/// class, and 3.14 does not.
#[test]
fn a_field_with_a_class_without_a_value_does_not_compile() {
    for field in [
        "prox: No | None",
        "prox: No | None = None",
        "segunda: Outra | None = None",
    ] {
        let what = unsupported(&formatdoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int
                {field}

            @dataclass
            class Outra:
                x: int
        "});
        assert_eq!(
            what, "a class in an annotation before its `class` statement",
            "{field}"
        );
    }
}

#[test]
fn a_field_in_quotes_takes_its_own_class() {
    same_as_cpython(
        "a_field_in_quotes_takes_its_own_class",
        indoc! {"
            from dataclasses import dataclass

            def f() -> None:
                print('default')
                return None

            @dataclass
            class No:
                v: int
                prox: 'No | None' = f()

            print(No(1, No(2)))
        "},
    );
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

/// `print` takes the `str` of each argument when it writes it, after it
/// evaluates every argument, and an error in a `str` comes after the
/// arguments before it are written.
#[test]
fn print_converts_each_argument_when_it_writes_it() {
    same_as_cpython(
        "print_converts_each_argument_when_it_writes_it",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int
                prox: 'No | None'

            def inc(n: No) -> int:
                n.v += 1
                return n.v

            def lista(n: int) -> No | None:
                if n == 0:
                    return None
                return No(n, lista(n - 1))

            a = No(1, None)
            b: No | None = a
            print(a, b, inc(a))
            print(1, lista(600))
        "},
    );
}

/// The checker keeps the narrowing of a field after a call that assigns
/// `None` to the field, and the object that the read gives behaves as
/// `None`.
#[test]
fn a_field_that_a_call_assigns_after_the_test_holds_none() {
    let start = indoc! {"
        from dataclasses import dataclass
        from enum import Enum

        class Cor(Enum):
            A = 1

        @dataclass
        class No:
            v: int
            prox: 'No | None'
            cor: Cor | None = None

        @dataclass
        class Par:
            a: No
            b: No

        def corta(n: No) -> None:
            n.prox = None
            n.cor = None

        def valor(n: No) -> int:
            return n.v

        t = No(1, No(2, None), Cor.A)
    "};
    same_as_cpython(
        "a_field_that_a_call_assigns_after_the_test_holds_none",
        &formatdoc! {"
            {start}
            if t.prox is not None and t.cor is not None:
                corta(t)
                print(t.prox, repr(t.prox), str(t.prox), f'{{t.prox}}')
                print(t.prox is None, t.prox == t.prox, t.prox == No(2, None), t.cor == Cor.A)
                print(not t.prox, Par(t.prox, No(3, None)))
                p = Par(t.prox, t.prox)
                print(p == Par(t.prox, t.prox), p == Par(No(2, None), t.prox))
                print(valor(t.prox))
        "},
    );
    same_as_cpython(
        "a_field_assignment_of_none_raises_attribute_error",
        &formatdoc! {"
            {start}
            if t.prox is not None:
                corta(t)
                p = t.prox
                p.v = 3
        "},
    );
    same_as_cpython(
        "an_augmented_assignment_of_none_raises_attribute_error",
        &formatdoc! {"
            {start}
            if t.prox is not None:
                corta(t)
                p = t.prox
                p.v += 3
        "},
    );
    same_as_cpython(
        "a_member_of_none_raises_attribute_error",
        &formatdoc! {"
            {start}
            if t.cor is not None:
                corta(t)
                print(t.cor.name)
        "},
    );
}

/// An augmented assignment to a field reads the object once, before the
/// field and the value.
#[test]
fn an_augmented_assignment_reads_the_object_once() {
    let start = indoc! {"
        from dataclasses import dataclass

        @dataclass
        class No:
            v: int
            prox: 'No | None'

        def corta(n: No) -> None:
            n.prox = None

        def f() -> int:
            global t
            print('f')
            t = No(10, None)
            return 5

        t = No(1, No(2, None))
        u = t
    "};
    same_as_cpython(
        "an_augmented_assignment_assigns_the_object_before_the_value",
        &formatdoc! {"
            {start}
            t.v += f()
            print(u, t)
            if u.prox is not None:
                u.prox.v -= f()
            print(u, t)

            def g() -> No:
                print('g')
                return u

            g().v *= f()
            print(u, t)
        "},
    );
    same_as_cpython(
        "an_augmented_assignment_of_none_raises_before_the_value",
        &formatdoc! {"
            {start}
            if t.prox is not None:
                corta(t)
                t.prox.v += f()
        "},
    );
}

/// The `str` of an object in a union runs with the site of the `print`.
#[test]
fn print_of_an_object_in_a_union_has_the_site_of_the_print() {
    same_as_cpython(
        "print_of_an_object_in_a_union_has_the_site_of_the_print",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                v: int
                prox: 'No | None'

            def lista(n: int) -> No:
                a = No(0, None)
                i = 0
                while i < n:
                    a = No(i, a)
                    i = i + 1
                return a

            def show(x: int | No) -> None:
                n = len('ab')
                print(x)

            show(lista(2000))
        "},
    );
}

/// The `print` of a union writes an object of a class that goes to a union
/// with the `str` of the class, and needs no `str` of another class.
#[test]
fn print_of_a_union_needs_the_str_of_the_classes_in_a_union() {
    same_as_cpython(
        "print_of_a_union_needs_the_str_of_the_classes_in_a_union",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class A:
                v: int

            @dataclass
            class B:
                x: int | str

            def show(x: int | A) -> None:
                print(x)

            def make(n: int) -> int | A:
                if n > 0:
                    return A(n)
                return n

            def text(s: int | str) -> None:
                print(s)

            show(make(1))
            show(make(0))
            text('a')
            print(B(1).x)
        "},
    );
}

/// The read of a field of a union after a test has the type of the test.
#[test]
fn a_test_narrows_a_field_of_a_union() {
    same_as_cpython(
        "a_test_narrows_a_field_of_a_union",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class P:
                nome: str | None
                idade: int | None
                peso: float | bool

            def f(p: P) -> int:
                if p.nome is not None and p.idade is not None:
                    s = p.nome
                    return len(s) + p.idade
                return 0

            print(f(P('ana', 3, 1.5)), f(P(None, 3, True)))
        "},
    );
}

/// A call that assigns a field of a union after the test that narrows the
/// field makes the read raise `TypeError`, where CPython goes on with the
/// value.
#[test]
fn a_field_of_a_union_that_a_call_assigns_after_the_test_raises() {
    let output = run(
        "a_field_of_a_union_that_a_call_assigns_after_the_test_raises",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class P:
                nome: str | None

            def limpa(p: P) -> None:
                p.nome = None

            p = P('ana')
            if p.nome is not None:
                limpa(p)
                print(p.nome)
                s = p.nome
        "},
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let end = indoc! {"
        \", line 13, in <module>
            print(p.nome)
                  ^^^^^^
        TypeError: the field 'nome' changed after the test that narrowed its type
    "};
    assert!(stderr.ends_with(end), "{stderr}");
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
            "a default that may be a list or an object of a dataclass",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

#[test]
fn a_field_of_a_list_holds_a_list() {
    same_as_cpython(
        "a_field_of_a_list_holds_a_list",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class Turma:
                nome: str
                notas: list[float]
                alunos: list[str] | None

            t = Turma('a', [7.5, 8.0], None)
            t.notas.append(9.0)
            t.notas[0] = 6.0
            print(t)
            print(t.notas, len(t.notas))
            t.alunos = ['Ana', 'Bia']
            print(repr(t))
            u = Turma('a', [6.0, 8.0, 9.0], ['Ana', 'Bia'])
            print(t == u, t != u)
            u.notas.pop()
            print(t == u)
            u.alunos = None
            print(t == u)
        "},
    );
}

#[test]
fn a_field_of_a_list_of_the_class_writes_and_compares_a_cycle() {
    same_as_cpython(
        "a_field_of_a_list_of_the_class_writes_and_compares_a_cycle",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class No:
                valor: int
                filhos: list['No']

            a = No(1, [])
            a.filhos.append(No(2, []))
            a.filhos.append(a)
            print(a)
            b = No(1, [No(2, [])])
            b.filhos.append(b)
            print(a == a, a.filhos[0] == b.filhos[0])
            print(a == b)
        "},
    );
}

#[test]
fn a_default_of_a_list_raises_value_error() {
    same_as_cpython(
        "a_default_of_a_list_raises_value_error",
        indoc! {"
            from dataclasses import dataclass

            print(1)

            @dataclass
            class P:
                x: int = 0
                xs: list[int] = []
                ys: list[int] = [1]
        "},
    );
}

#[test]
fn an_operator_with_assignment_on_a_field_of_a_list_does_not_compile() {
    assert_eq!(
        unsupported(indoc! {"
            from dataclasses import dataclass

            @dataclass
            class P:
                xs: list[int]

            p = P([1])
            p.xs += [2]
        "}),
        "an operator with assignment on a list",
    );
}

#[test]
fn eq_of_a_union_with_a_dataclass_compares_the_fields() {
    same_as_cpython(
        "eq_of_a_union_with_a_dataclass_compares_the_fields",
        indoc! {"
            from dataclasses import dataclass


            @dataclass
            class P:
                x: int


            @dataclass
            class Q:
                x: int


            @dataclass
            class R:
                p: P | int
                ps: list['P | int']


            def eq(a: P | Q | int, b: P | Q | int) -> bool:
                return a == b


            print(eq(P(1), P(1)), eq(P(1), P(2)), eq(P(1), Q(1)), eq(P(1), 1), eq(1, 1))
            print(P(1) != P(1), Q(1) == Q(1))
            print(R(P(1), [P(2), 3]) == R(P(1), [P(2), 3]), R(P(1), []) == R(P(2), []))
            print(R(1, [P(2)]) == R(1, [P(3)]), R(1, [1]) == R(1, [1]))
            ps: list[P | int] = [P(1), 2, P(3), P(1)]
            qs: list[Q | int] = [Q(1), 2]
            print(P(1) in ps, P(2) in ps, 2 in ps, Q(1) in qs)
            print(ps.count(P(1)), ps.index(P(3)), qs.index(Q(1)))
            ps.remove(P(3))
            print(ps, ps == [P(1), 2, P(1)], ps != [P(1), 2, P(1)])
            ls: list[list[P | int]] = [[P(1)], [2]]
            ms: list[list[P | int]] = [[P(1)], [2]]
            ns: list[list[P | int]] = [[P(2)], [2]]
            one: list[P | int] = [P(1)]
            print(ls == ms, ls == ns, one in ls)
            us: list[P | Q | int] = [P(1), Q(1), 1]
            print(us.count(Q(1)), us.index(1), us == [P(1), Q(1), 1])
        "},
    );
}

#[test]
fn a_dataclass_calls_its_methods() {
    same_as_cpython(
        "a_dataclass_calls_its_methods",
        indoc! {"
            from dataclasses import dataclass

            def f(x: int) -> int:
                print('default', x)
                return x

            @dataclass
            class Tempo:
                horas: int
                minutos: int = f(0)

                def total(self) -> int:
                    return self.horas * 60 + self.minutos

                def soma(self, outro: 'Tempo', extra: int = f(5)) -> 'Tempo':
                    t = self.total() + outro.total() + extra
                    return Tempo(t // 60, t % 60)

                def zera(self) -> None:
                    self.horas = 0
                    self.minutos = 0

            @dataclass(frozen=True)
            class Ponto:
                x: float
                y: float

                def norma2(self) -> float:
                    return self.x * self.x + self.y * self.y

            a = Tempo(1, 30)
            b = a.soma(Tempo(2), extra=10)
            print(a.total(), b, b.total())
            b.zera()
            print(b, Ponto(3.0, 4.0).norma2())
            print(a.soma(Tempo(0, 1 // f(0))))
        "},
    );
}

#[test]
fn a_special_method_in_a_dataclass_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            from dataclasses import dataclass

            @dataclass
            class Tempo:
                horas: int

                def __repr__(self) -> str:
                    return 'tempo'
        "}),
        "a special method in a dataclass"
    );
}
