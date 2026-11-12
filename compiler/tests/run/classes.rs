//! The classes without a decorator.

use std::path::Path;

use indoc::{formatdoc, indoc};

use super::{compile, run, same_as_cpython, unsupported};

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
fn a_method_takes_self_and_the_arguments() {
    same_as_cpython(
        "a_method_takes_self_and_the_arguments",
        indoc! {"
            class Point:
                x: int
                y: int

                def __init__(self, x: int, y: int) -> None:
                    self.x = x
                    self.y = y

                def scale(self, factor: int) -> \"Point\":
                    return Point(self.x * factor, self.y * factor)

                def move(self, dx: int = 1, dy: int = 0) -> None:
                    self.x += dx
                    self.y += dy

                def norm1(self) -> int:
                    return abs(self.x) + abs(self.y)

                def far(self, limit: int) -> bool:
                    return self.norm1() > limit

            def f(x: int) -> int:
                print(x)
                return x

            p = Point(3, 4).scale(2)
            print(p.x, p.y)
            p.move()
            p.move(dy=f(2), dx=f(1))
            p.move(f(3), f(4))
            print(p.x, p.y, p.norm1(), p.far(10), p.far(100))
        "},
    );
}

#[test]
fn an_exception_in_a_method_has_the_frame_of_the_method() {
    same_as_cpython(
        "an_exception_in_a_method_has_the_frame_of_the_method",
        indoc! {"
            class Conta:
                saldo: int

                def __init__(self) -> None:
                    self.saldo = 0

                def saca(self, valor: int) -> None:
                    if valor > self.saldo:
                        raise ValueError('saldo insuficiente')
                    self.saldo -= valor

                def esvazia(self) -> None:
                    self.saca(self.saldo + 1)

            c = Conta()
            c.saca(0)
            c.esvazia()
        "},
    );
}

#[test]
fn a_method_of_none_raises_attribute_error() {
    same_as_cpython(
        "a_method_of_none_raises_attribute_error",
        indoc! {"
            class No:
                proximo: 'No | None'

                def __init__(self) -> None:
                    self.proximo = None

                def corta(self) -> None:
                    self.proximo = None

                def anda(self) -> None:
                    if self.proximo is not None:
                        self.corta()
                        self.proximo.corta()

            a = No()
            a.proximo = No()
            a.anda()
            print('ok')
        "},
    );
}

#[test]
fn a_special_method_other_than_init_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                def __hash__(self) -> int:
                    return 1
        "}),
        "this special method"
    );
}

/// A method called in `__init__` may read any field.
#[test]
fn a_method_call_in_init_before_each_field_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ponto:
                x: int
                y: int

                def __init__(self) -> None:
                    self.x = 0
                    self.mostra()
                    self.y = 0

                def mostra(self) -> None:
                    print(self.x, self.y)
        "}),
        "self before __init__ assigns each field"
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

#[test]
fn an_object_writes_its_str_and_its_repr() {
    same_as_cpython(
        "an_object_writes_its_str_and_its_repr",
        indoc! {"
            from dataclasses import dataclass
            from enum import Enum

            class Fracao:
                num: int
                den: int

                def __init__(self, num: int, den: int) -> None:
                    self.num = num
                    self.den = den

                def __str__(self) -> str:
                    return str(self.num) + '/' + str(self.den)

                def __repr__(self) -> str:
                    return 'Fracao(' + str(self.num) + ', ' + str(self.den) + ')'

            class SoRepr:
                def __repr__(self) -> str:
                    return 'SoRepr()'

            @dataclass
            class Ponto:
                x: int
                y: int

                def __repr__(self) -> str:
                    return f'<{self.x}, {self.y}>'

            @dataclass
            class Nome:
                texto: str

                def __str__(self) -> str:
                    return self.texto

            class Cor(Enum):
                A = 1
                B = 2

                def __str__(self) -> str:
                    return self.name.lower()

            class Tam(Enum):
                P = 1

                def __repr__(self) -> str:
                    return 'Tam.P!'

            f = Fracao(1, 2)
            g: Fracao | None = f
            print(f, [f], repr(f), str(f), f'{f}', g)
            print(SoRepr(), [SoRepr()], str(SoRepr()))
            print(Ponto(1, 2), [Ponto(3, 4)], Nome('ana'), [Nome('bia')])
            print(Cor.A, [Cor.B], f'{Cor.A}', Tam.P, [Tam.P])
        "},
    );
}

/// `print` writes the arguments before the one whose `__str__` raises.
#[test]
fn an_exception_in_str_has_the_frame_of_str() {
    same_as_cpython(
        "an_exception_in_str_has_the_frame_of_str",
        indoc! {"
            class Ruim:
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n

                def __str__(self) -> str:
                    return str(10 // self.n)

            print(Ruim(1), Ruim(2))
            print('a', Ruim(0), 'b')
        "},
    );
}

#[test]
fn a_conversion_with_parameters_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class Ruim:
                def __str__(self, x: int = 0) -> str:
                    return 'ruim'

            print(Ruim())
        "}),
        "a conversion with these parameters"
    );
}

/// A subclass has the fields of its base first, takes the methods of the
/// base, and takes the `__init__` of the base when it has none.
#[test]
fn a_subclass_takes_the_fields_and_the_methods_of_its_base() {
    same_as_cpython(
        "a_subclass_takes_the_fields_and_the_methods_of_its_base",
        indoc! {"
            class Animal:
                nome: str
                patas: int

                def __init__(self, nome: str, patas: int) -> None:
                    self.nome = nome
                    self.patas = patas

                def descricao(self) -> str:
                    return self.nome + ' com ' + str(self.patas) + ' patas'

            class Cachorro(Animal):
                raca: str

                def __init__(self, nome: str, raca: str) -> None:
                    self.nome = nome
                    self.patas = 4
                    self.raca = raca

                def late(self) -> str:
                    return self.nome + ' late'

            class Gato(Animal):
                pass

            class Filhote(Cachorro):
                nome: str

            c = Cachorro('Rex', 'vira-lata')
            g = Gato('Mia', 4)
            f = Filhote('Bob', 'poodle')
            print(c.descricao(), c.late(), c.raca)
            print(g.descricao(), f.late(), f.descricao(), f.raca)
            c.patas = 3
            print(c.descricao())
        "},
    );
}

/// An object of a subclass writes its own class, through a value of its
/// class, of the base and of `object`. CPython writes the address too.
#[test]
fn an_object_of_a_subclass_writes_its_own_class() {
    let output = run(
        "an_object_of_a_subclass_writes_its_own_class",
        indoc! {"
            class A:
                def nome(self) -> str:
                    return str(self)

            class B(A):
                pass

            class C:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def __repr__(self) -> str:
                    return 'C(' + str(self.x) + ')'

            class D(C):
                pass

            def mostra(x: object) -> None:
                print(x)

            def uniao(n: int) -> A | int:
                if n > 0:
                    return B()
                return n

            print(A(), B(), A().nome(), B().nome())
            print(repr(B()), f'{B()}', [D(1), D(2)])
            print(C(1), D(2), str(D(3)))
            mostra(B())
            mostra(D(4))
            print(uniao(1), uniao(0))
        "},
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        indoc! {"
            <__main__.A object> <__main__.B object> <__main__.A object> <__main__.B object>
            <__main__.B object> <__main__.B object> [C(1), C(2)]
            C(1) C(2) C(3)
            <__main__.B object>
            C(4)
            <__main__.B object> 0
        "}
    );
}

#[test]
fn a_subclass_outside_the_accepted_form_is_unsupported() {
    for (source, what) in [
        (
            indoc! {"
                class A:
                    def f(self, x: int = 1) -> int:
                        return x

                class B(A):
                    def f(self, x: int = 1) -> int:
                        return 2
            "},
            "an override of a method with defaults",
        ),
        (
            indoc! {"
                class A:
                    def f(self, x: int) -> int:
                        return x

                class B(A):
                    def f(self, x: int, y: int = 2) -> int:
                        return y
            "},
            "an override of a method with defaults",
        ),
        (
            indoc! {"
                class A:
                    def f(self, x: int) -> int:
                        return x

                class B(A):
                    def f(self, x: int | None) -> int:
                        return 2
            "},
            "an override with other parameters or result",
        ),
        (
            indoc! {"
                class A:
                    x: int

                    def __init__(self) -> None:
                        self.x = 1

                class B(A):
                    y: int
            "},
            "a field that __init__ may not assign",
        ),
        (
            indoc! {"
                class A:
                    x: int

                    def __init__(self) -> None:
                        self.x = 1

                class B(A):
                    def __init__(self) -> None:
                        print(self.x)
                        self.x = 2
            "},
            "a field that __init__ may read before it assigns it",
        ),
        (
            indoc! {"
                from enum import Enum

                class A(Enum):
                    pass

                class B(A):
                    X = 1
            "},
            "an Enum as a base",
        ),
        (
            indoc! {"
                from dataclasses import dataclass

                @dataclass
                class A:
                    x: int

                class B(A):
                    pass
            "},
            "a dataclass as a base",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

/// `super().__init__(...)` assigns the fields of the base, and
/// `super().m()` calls the method of the base, in two levels.
#[test]
fn super_calls_the_method_of_the_base() {
    same_as_cpython(
        "super_calls_the_method_of_the_base",
        indoc! {"
            class Animal:
                nome: str

                def __init__(self, nome: str) -> None:
                    print('Animal', nome)
                    self.nome = nome

                def fala(self) -> str:
                    return self.nome + ' faz barulho'

            class Cachorro(Animal):
                raca: str

                def __init__(self, nome: str, raca: str = 'vira-lata') -> None:
                    super().__init__(nome)
                    self.raca = raca

                def late(self) -> str:
                    return super().fala() + ' e late'

            class Filhote(Cachorro):
                idade: int

                def __init__(self, nome: str, idade: int) -> None:
                    self.idade = idade
                    super().__init__(nome, raca='poodle')

                def chora(self) -> str:
                    return super().late() + ', ' + super().fala()

            class Vazio:
                def __init__(self) -> None:
                    super().__init__()

            class Outro(Vazio):
                pass

            f = Filhote('Bob', 1)
            print(f.nome, f.raca, f.idade)
            print(f.chora())
            print(Cachorro('Rex').late())
            Outro()
        "},
    );
}

/// An exception in the method of the base has the frame of the call
/// through `super()`.
#[test]
fn an_exception_through_super_has_the_frames_of_both_methods() {
    same_as_cpython(
        "an_exception_through_super_has_the_frames_of_both_methods",
        indoc! {"
            class A:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = 10 // x

            class B(A):
                def __init__(self, x: int) -> None:
                    super().__init__(x)

            print(B(2).x)
            B(0)
        "},
    );
}

#[test]
fn super_outside_the_accepted_form_is_unsupported() {
    for (source, what) in [
        (
            indoc! {"
                class A:
                    x: int

                    def __init__(self) -> None:
                        self.x = 1

                class B(A):
                    y: int

                    def __init__(self) -> None:
                        super().__init__()
                        print(super().__init__)
                        self.y = 2
            "},
            "self before __init__ assigns each field",
        ),
        (
            indoc! {"
                class A:
                    x: int

                    def __init__(self) -> None:
                        self.x = 1
                        print(self)

                class B(A):
                    y: int

                    def __init__(self) -> None:
                        super().__init__()
                        self.y = 2
            "},
            "self before __init__ assigns each field",
        ),
        (
            indoc! {"
                class A:
                    def g(self) -> str:
                        return super().__repr__()
            "},
            "this attribute of super()",
        ),
        (
            indoc! {"
                class A:
                    def f(self) -> int:
                        return 1

                class B(A):
                    def g(self) -> int:
                        return super(B, self).f()
            "},
            "super with arguments",
        ),
    ] {
        assert_eq!(unsupported(source), what, "{source}");
    }
}

/// A value of a class holds an object of a subclass, and the checker
/// narrows a read after an assignment to the class of the object.
#[test]
fn an_object_of_a_subclass_is_a_value_of_its_base() {
    same_as_cpython(
        "an_object_of_a_subclass_is_a_value_of_its_base",
        indoc! {"
            class Animal:
                nome: str

                def __init__(self, nome: str) -> None:
                    self.nome = nome

                def descricao(self) -> str:
                    return 'animal ' + self.nome

            class Cachorro(Animal):
                def late(self) -> str:
                    return self.nome + ' late'

            class Gato(Animal):
                pass

            class Filhote(Cachorro):
                pass

            class Dono:
                pet: Animal | None

                def __init__(self) -> None:
                    self.pet = None

            def descreve(a: Animal) -> str:
                return a.descricao()

            def novo(nome: str) -> Cachorro:
                if nome == 'Bidu':
                    return Filhote(nome)
                return Cachorro(nome)

            def primeiro(animais: list[Animal]) -> Animal | None:
                if len(animais) > 0:
                    return animais[0]
                return None

            a: Animal = Cachorro('Rex')
            print(a.late(), descreve(a), descreve(Gato('Mia')))
            animais: list[Animal] = [Cachorro('Bob'), Gato('Tom')]
            animais.append(Cachorro('Max'))
            for b in animais:
                print(b.descricao(), b is animais[0])
            p = primeiro(animais)
            if p is not None:
                print(p.nome)
            d = Dono()
            d.pet = Cachorro('Fido')
            print(d.pet.late())
            x: Animal | None = None
            if len(animais) > 2:
                x = Gato('Lua')
            if x is not None:
                print(x.descricao())
            c: Animal = novo('Bidu')
            print(c.late(), novo('Rex').late())
            d.pet = novo('Bidu')
            print(d.pet.late())
        "},
    );
}

/// The number of `ref.test`s in `module`, after a check that no test and
/// no cast names a type that is not final.
fn tests_on_final_types(module: &compiler::Module) -> usize {
    use wasmparser::{HeapType, Operator, Parser, Payload, UnpackedIndex};

    let mut open = Vec::new();
    let mut types = 0;
    let mut tests = 0;
    let named = |heap: HeapType| match heap {
        HeapType::Concrete(UnpackedIndex::Module(index)) => Some(index),
        _ => None,
    };
    for payload in Parser::new(0).parse_all(&module.bytes) {
        match payload.expect("a valid module") {
            Payload::TypeSection(reader) => {
                for group in reader {
                    for ty in group.expect("a rec group").types() {
                        if !ty.is_final {
                            open.push(types);
                        }
                        types += 1;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let mut ops = body.get_operators_reader().expect("the code");
                while !ops.eof() {
                    let heap = match ops.read().expect("an instruction") {
                        Operator::RefTestNonNull { hty } | Operator::RefTestNullable { hty } => {
                            tests += 1;
                            Some(hty)
                        }
                        Operator::RefCastNonNull { hty } | Operator::RefCastNullable { hty } => {
                            Some(hty)
                        }
                        Operator::BrOnCast { to_ref_type, .. }
                        | Operator::BrOnCastFail { to_ref_type, .. } => {
                            Some(to_ref_type.heap_type())
                        }
                        _ => None,
                    };
                    if let Some(index) = heap.and_then(named) {
                        assert!(!open.contains(&index), "a test of the open type {index}");
                    }
                }
            }
            _ => {}
        }
    }
    tests
}

/// A call through a value of a class has a test for each class that the
/// program makes, and none for a class that it never makes.
#[test]
fn a_call_tests_the_classes_that_the_program_makes() {
    let program = |made: &str| {
        formatdoc! {"
            class A:
                def nome(self) -> str:
                    return 'a'

            class B(A):
                pass

            class C(A):
                pass

            class D(C):
                pass

            def f(x: A) -> None:
                print(x, x.nome())

            {made}
        "}
    };
    let tests = |made: &str| {
        let module =
            compile(&program(made), Path::new("tests.py")).expect("the program to compile");
        tests_on_final_types(&module)
    };
    let one = tests("f(B())");
    assert_eq!(tests("f(B())\nf(C())"), one + 1);
    assert_eq!(tests("f(B())\nf(C())\nf(D())"), one + 2);
    assert_eq!(tests("f(D())"), one);
}

/// A method that a subclass overrides runs the body of the class of the
/// object, through a value of the root, of a class in the middle and of a
/// leaf. The middle class is made in one program and not in the other,
/// and the root is never made.
#[test]
fn a_call_runs_the_method_of_the_class_of_the_object() {
    for (name, middle) in [
        ("a_call_runs_the_method_of_the_class_of_the_object", "B()"),
        (
            "a_call_runs_the_method_of_a_class_that_the_program_makes",
            "C()",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                class A:
                    def nome(self) -> str:
                        return 'A'

                    def fala(self) -> str:
                        return 'sou ' + self.nome()

                    def __str__(self) -> str:
                        return '<' + self.nome() + '>'

                class B(A):
                    def nome(self) -> str:
                        return 'B'

                class C(B):
                    def nome(self) -> str:
                        return 'C de ' + super().nome()

                class D(B):
                    def fala(self) -> str:
                        return 'D fala'

                class E(A):
                    pass

                def pela_raiz(a: A) -> str:
                    return a.nome() + ' ' + a.fala()

                def pelo_meio(b: B) -> str:
                    return b.nome() + ' ' + b.fala()

                def pela_folha(c: C) -> str:
                    return c.nome() + ' ' + c.fala()

                meio = {middle}
                raizes: list[A] = [meio, C(), D(), E()]
                for a in raizes:
                    print(pela_raiz(a), a)
                meios: list[B] = [meio, C(), D()]
                for b in meios:
                    print(pelo_meio(b), b)
                print(pela_folha(C()))
            "},
        );
    }
}

/// An `__init__` of the base that calls a method that the subclass
/// overrides runs the method of the subclass, which reads a field of the
/// subclass that its `__init__` assigns before `super().__init__`.
#[test]
fn the_init_of_the_base_calls_a_method_of_the_subclass() {
    same_as_cpython(
        "the_init_of_the_base_calls_a_method_of_the_subclass",
        indoc! {"
            class Base:
                texto: str

                def __init__(self) -> None:
                    self.texto = ''
                    self.texto = self.descreve()

                def descreve(self) -> str:
                    return 'base'

            class Filha(Base):
                n: int

                def __init__(self, n: int) -> None:
                    self.n = n
                    super().__init__()

                def descreve(self) -> str:
                    return 'filha ' + str(self.n) + ' ' + super().descreve()

            print(Base().texto, Filha(3).texto)
        "},
    );
}

#[test]
fn the_init_of_a_subclass_assigns_its_fields_before_an_escape_of_the_base() {
    assert_eq!(
        unsupported(indoc! {"
            class Base:
                texto: str

                def __init__(self) -> None:
                    self.texto = ''
                    self.texto = self.descreve()

                def descreve(self) -> str:
                    return 'base'

            class Filha(Base):
                n: int

                def __init__(self, n: int) -> None:
                    super().__init__()
                    self.n = n

                def descreve(self) -> str:
                    return str(self.n)
        "}),
        "self before __init__ assigns each field"
    );
}

/// The call tests each class that the program makes, with 8 and with 16
/// classes.
#[test]
fn a_call_dispatches_among_many_classes() {
    for count in [8, 16] {
        let classes: String = (1..count)
            .map(|i| {
                formatdoc! {"
                    class C{i}(C0):
                        def valor(self) -> int:
                            return {i}

                "}
            })
            .collect();
        let made: Vec<String> = (0..count).map(|i| format!("C{i}()")).collect();
        same_as_cpython(
            &format!("a_call_dispatches_among_{count}_classes"),
            &formatdoc! {"
                class C0:
                    def valor(self) -> int:
                        return 0

                {classes}
                def soma(objetos: list[C0]) -> int:
                    total = 0
                    for o in objetos:
                        total = total * 3 + o.valor()
                    return total

                objetos: list[C0] = [{made}]
                print(soma(objetos), soma(objetos[::-1]))
            ", made = made.join(", ")},
        );
    }
}

/// A second `class` of a name gives the name a new class, and the code of
/// the module after it builds objects of the new class.
#[test]
fn a_second_class_replaces_the_class_of_the_name() {
    same_as_cpython(
        "a_second_class_replaces_the_class_of_the_name",
        indoc! {"
            from dataclasses import dataclass

            @dataclass
            class P:
                x: int

            p = P(1)
            print(p.x, p)

            class P:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

                def dobro(self) -> int:
                    return 2 * self.x

                def __repr__(self) -> str:
                    return f'P({self.x})'

            class Q(P):
                pass

            q = P(3)
            print(q, q.dobro(), Q(4).dobro(), isinstance(Q(1), P))
            print(p.x, p)
        "},
    );
}

#[test]
fn a_call_of_a_name_of_two_classes_in_a_function_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class P:
                pass

            def novo() -> None:
                P()

            class P:
                pass
        "}),
        "a call of a name of two classes"
    );
}

/// A call of `__init__` through an object runs the `__init__` of the
/// exact class of the object.
#[test]
fn an_explicit_call_of_init_runs_the_init_of_the_object() {
    same_as_cpython(
        "an_explicit_call_of_init_runs_the_init_of_the_object",
        indoc! {"
            class A:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

            class B(A):
                def __init__(self, x: int) -> None:
                    super().__init__(x * 10)

            def reset(a: A) -> None:
                a.__init__(5)
                print(a.x)

            reset(A(1))
            reset(B(1))
        "},
    );
}

#[test]
fn a_call_of_init_that_a_subclass_overrides_otherwise_is_unsupported() {
    assert_eq!(
        unsupported(indoc! {"
            class A:
                x: int

                def __init__(self, x: int) -> None:
                    self.x = x

            class B(A):
                def __init__(self) -> None:
                    super().__init__(0)

            def reset(a: A) -> None:
                a.__init__(5)

            reset(B())
        "}),
        "a call of __init__ that a subclass overrides otherwise"
    );
}

/// An annotation in `__init__` reads its value alone. A bare annotation
/// of a field reads nothing, and a local may take a field that
/// `__init__` assigned before.
#[test]
fn an_annotation_in_init_reads_its_value_alone() {
    same_as_cpython(
        "an_annotation_in_init_reads_its_value_alone",
        indoc! {"
            class A:
                x: int
                y: int

                def __init__(self) -> None:
                    self.x: int
                    self.x = 1
                    n: int = self.x
                    self.y = n + 1

            a = A()
            print(a.x, a.y)
        "},
    );
}
