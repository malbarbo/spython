//! `isinstance` with a class of the program or a builtin type.

use indoc::indoc;

use super::{same_as_cpython, unsupported};

/// A test of an open class tests the exact type of each class that
/// descends from it, and the checker narrows the value after the test.
#[test]
fn isinstance_tests_a_class_of_the_program() {
    same_as_cpython(
        "isinstance_tests_a_class_of_the_program",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum

            class Animal:
                nome: str

                def __init__(self, nome: str) -> None:
                    self.nome = nome

            class Cachorro(Animal):
                def late(self) -> str:
                    return self.nome + ' late'

            class Gato(Animal):
                pass

            @dataclass
            class P:
                x: int

            class Cor(Enum):
                A = 1

            def descreve(a: Animal) -> str:
                if isinstance(a, Cachorro):
                    return a.late()
                return 'outro'

            def qual(v: object) -> str:
                if isinstance(v, Animal):
                    return 'animal ' + v.nome
                if isinstance(v, P):
                    return 'p ' + str(v.x)
                if isinstance(v, Cor):
                    return v.name
                return 'outro'

            print(descreve(Cachorro('rex')), descreve(Gato('tom')), descreve(Animal('x')))
            print(qual(Gato('g')), qual(P(3)), qual(Cor.A), qual(None), qual(3))
            c: Animal | None = None
            print(isinstance(c, Animal), isinstance(c, Cachorro))
            c = Cachorro('a')
            print(isinstance(c, Animal), isinstance(c, Cachorro), isinstance(c, Gato))
            print(isinstance(c, P), isinstance(Gato('a'), Animal), isinstance(P(1), P))
        "},
    );
}

/// A test of a class for a value of the class or `None` tests only for
/// `None`.
#[test]
fn isinstance_of_an_optional_object_tests_for_none() {
    same_as_cpython(
        "isinstance_of_an_optional_object_tests_for_none",
        indoc! {"
            class Animal:
                nome: str

                def __init__(self, nome: str) -> None:
                    self.nome = nome

            class Gato(Animal):
                pass

            def animal(a: Animal | None) -> bool:
                return isinstance(a, Animal)

            def gato(g: Gato | None) -> bool:
                return isinstance(g, Animal)

            print(animal(None), animal(Animal('a')), animal(Gato('g')))
            print(gato(None), gato(Gato('g')))
        "},
    );
}

/// A `bool` is an `int`, and an `int` is not a `float`.
#[test]
fn isinstance_tests_a_builtin_type() {
    same_as_cpython(
        "isinstance_tests_a_builtin_type",
        indoc! {"
            def tipo(v: object) -> str:
                if isinstance(v, bool):
                    return 'bool'
                if isinstance(v, float):
                    return 'float ' + str(v + 1.0)
                if isinstance(v, str):
                    return 'str ' + v.upper()
                if v is None:
                    return 'none'
                return 'outro'

            def valor(n: int) -> int | bool | str | None:
                if n > 2:
                    return n
                if n > 1:
                    return True
                if n > 0:
                    return 'um'
                return None

            print(tipo(True), tipo(1.5), tipo('a'), tipo(None), tipo(3))
            print(isinstance(True, int), isinstance(1, bool), isinstance(1, float))
            print(isinstance(1.0, float), isinstance('a', str), isinstance(None, str))
            print(isinstance([1], list), isinstance(3, object), isinstance(None, object))
            for i in range(4):
                v = valor(i)
                print(isinstance(v, int), isinstance(v, bool), isinstance(v, str))
            s: str | None = None
            xs: list[int] | None = None
            print(isinstance(s, str), isinstance(xs, list))
            s = 'a'
            xs = []
            print(isinstance(s, str), isinstance(xs, list))
        "},
    );
}

/// The value runs before the read of the class, which raises `NameError`
/// before the `class` statement.
#[test]
fn isinstance_reads_the_class_after_the_value() {
    same_as_cpython(
        "isinstance_reads_the_class_after_the_value",
        indoc! {"
            def valor() -> object:
                print('valor')
                return 1

            def f() -> bool:
                return isinstance(valor(), C)

            print(f())

            class C:
                pass
        "},
    );
}

#[test]
fn isinstance_outside_the_accepted_form_is_unsupported() {
    for (source, what) in [
        (
            indoc! {"
                def f(v: object) -> bool:
                    return isinstance(v, int)
            "},
            "isinstance of an object with int",
        ),
        (
            indoc! {"
                def f(v: object) -> bool:
                    return isinstance(v, (int, str))
            "},
            "isinstance with this class",
        ),
        (
            indoc! {"
                def f(v: object) -> bool:
                    return isinstance(v, dict)
            "},
            "isinstance with this class",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}
