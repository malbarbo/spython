//! The classes without a decorator.

use indoc::indoc;

use super::{run, same_as_cpython, unsupported};

#[test]
fn a_class_with_fields_and_init_makes_objects() {
    same_as_cpython(
        "a_class_with_fields_and_init_makes_objects",
        indoc! {"
            class Conta:
                \"\"\"Uma conta.\"\"\"

                titular: str
                saldo: float
                itens: list[int]
                proxima: 'Conta | None'

                def __init__(self, titular: str, saldo: float = 0.0) -> None:
                    self.titular = titular
                    self.saldo = saldo
                    self.itens = [1, 2]
                    self.proxima = None
                    print('init', self.titular, self.saldo)

            def f(x: str) -> str:
                print(x)
                return x

            a = Conta(f('ana'))
            b = Conta(saldo=2.5, titular=f('bia'))
            a.proxima = b
            a.saldo += 10
            print(a.titular, a.saldo, a.itens, b.saldo)
            p = a.proxima
            if p is not None:
                print(p.titular)
            print(a is b, a == a, a != b)
        "},
    );
}

#[test]
fn a_class_without_init_takes_no_argument() {
    same_as_cpython(
        "a_class_without_init_takes_no_argument",
        indoc! {"
            class Vazia:
                pass

            v = Vazia()
            w = Vazia()
            print(v is w, v is v)
        "},
    );
}

/// The default of a parameter of `__init__` runs with the `class`
/// statement, in the frame of the body of the class.
#[test]
fn the_defaults_of_init_run_in_the_body_of_the_class() {
    same_as_cpython(
        "the_defaults_of_init_run_in_the_body_of_the_class",
        indoc! {"
            def f(x: int) -> int:
                print('default', x)
                return 10 // x

            print('antes')

            class Ponto:
                x: int

                def __init__(self, x: int = f(2)) -> None:
                    self.x = x

            print(Ponto().x, Ponto(3).x)

            class Ruim:
                x: int

                def __init__(self, x: int = f(0)) -> None:
                    self.x = x
        "},
    );
}

#[test]
fn an_exception_in_init_has_the_frame_of_init() {
    same_as_cpython(
        "an_exception_in_init_has_the_frame_of_init",
        indoc! {"
            class Fracao:
                num: int
                den: int

                def __init__(self, num: int, den: int) -> None:
                    self.num = num
                    self.den = den
                    print(num // den)

            Fracao(1, 1)
            Fracao(1, 0)
        "},
    );
}

/// The program writes no address, which changes from one run of CPython
/// to the next.
#[test]
fn an_object_without_str_writes_its_class() {
    let output = run(
        "an_object_without_str_writes_its_class",
        indoc! {"
            class Vazia:
                pass

            v: Vazia | None = Vazia()
            print(Vazia(), [Vazia()], v)
        "},
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "<__main__.Vazia object> [<__main__.Vazia object>] <__main__.Vazia object>\n"
    );
}

#[test]
fn a_name_error_in_a_method_suggests_a_field_of_self() {
    same_as_cpython(
        "a_name_error_in_a_method_suggests_a_field_of_self",
        indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    print(valor)

            if len('a') > 1:
                valor = 1
            Ponto(1)
        "},
    );
}

/// Each path assigns each field before a read and before `self` goes
/// elsewhere.
#[test]
fn init_assigns_the_fields_in_each_path() {
    same_as_cpython(
        "init_assigns_the_fields_in_each_path",
        indoc! {"
            def mostra(p: 'Ponto') -> None:
                print('mostra', p.x, p.y, p.nome)

            class Ponto:
                x: int
                y: int
                nome: str

                def __init__(self, x: int) -> None:
                    if x > 0:
                        self.x = x
                        self.y = self.x * 2
                    elif x < 0:
                        self.y = -x
                        self.x = self.y
                    else:
                        raise ValueError('zero')
                    i = 0
                    while i < 2:
                        self.x += 1
                        i += 1
                    self.nome = 'p' + str(self.x)
                    mostra(self)
                    outro = self
                    print(outro.nome)

            Ponto(1)
            Ponto(-3)
            Ponto(0)
        "},
    );
}

#[test]
fn a_field_that_init_may_not_assign_is_unsupported() {
    let cases = [
        indoc! {"
            class Ponto:
                x: int
                y: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    if x > 0:
                        self.y = x
        "},
        indoc! {"
            class Ponto:
                x: int
                y: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    while x > 0:
                        self.y = x
                        x -= 1
        "},
        indoc! {"
            class Ponto:
                x: int
                y: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    if x > 0:
                        return
                    self.y = x
        "},
        indoc! {"
            class Ponto:
                x: int
        "},
    ];
    for source in cases {
        assert_eq!(unsupported(source), "a field that __init__ may not assign");
    }
}

#[test]
fn a_read_of_a_field_before_its_assignment_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                x: int
                y: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    if x > 0:
                        self.y = 1
                    self.x = self.y
        "}),
        "a field that __init__ may read before it assigns it"
    );
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x += x
        "}),
        "a field that __init__ may read before it assigns it"
    );
}

#[test]
fn self_before_each_field_has_a_value_is_unsupported() {
    let cases = [
        indoc! {"
            def f(p: 'Ponto') -> None:
                pass

            class Ponto:
                x: int
                y: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    f(self)
                    self.y = x
        "},
        indoc! {"
            class Ponto:
                x: int
                proximo: 'Ponto | None'

                def __init__(self, x: int) -> None:
                    outro = self
                    self.x = x
                    self.proximo = None
        "},
    ];
    for source in cases {
        assert_eq!(
            unsupported(source),
            "self before __init__ assigns each field"
        );
    }
}

#[test]
fn an_assignment_to_the_receiver_of_init_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x
                    self = Ponto(1)
        "}),
        "an assignment to self in __init__"
    );
}

#[test]
fn an_attribute_without_an_annotation_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                def __init__(self, x: int) -> None:
                    self.x = x
        "}),
        "the attribute x without an annotation in the class"
    );
}

#[test]
fn a_method_other_than_init_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                def m(self) -> None:
                    pass
        "}),
        "a method other than __init__"
    );
}

#[test]
fn a_name_of_a_field_in_init_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                x: int

                def __init__(self) -> None:
                    self.x = x

            if len('a') > 1:
                x = 1
        "}),
        "the name of a field in __init__"
    );
}
