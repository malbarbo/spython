mod testlib;

use indoc::indoc;
use testlib::run;

#[test]
fn dataclass_basic() {
    run(indoc! {"
        from dataclasses import dataclass

        @dataclass
        class Point:
            x: int
            y: int

        p: Point = Point(10, 20)
        assert p.x == 10
        assert p.y == 20
    "});
}

#[test]
fn dataclass_nested() {
    run(indoc! {"
        from dataclasses import dataclass

        @dataclass
        class Point:
            x: int
            y: int

        @dataclass
        class Rect:
            p1: Point
            p2: Point

        r: Rect = Rect(Point(1, 2), Point(3, 4))
        assert r.p1.x == 1
        assert r.p2.y == 4
    "});
}

#[test]
fn enum_basic() {
    run(indoc! {"
        from enum import Enum, auto

        class Cor(Enum):
            VERDE = auto()
            AMARELO = auto()
            VERMELHO = auto()

        c: int = Cor.AMARELO
        assert c == 2
        assert Cor.VERDE == 1
        assert Cor.VERMELHO == 3
    "});
}

#[test]
fn match_basic() {
    run(indoc! {"
        x: int = 2
        res: int = 0
        match x:
            case 1:
                res = 10
            case 2:
                res = 20
            case _:
                res = 30
        assert res == 20
    "});
}

#[test]
fn match_enum() {
    run(indoc! {"
        from enum import Enum, auto

        class Cor(Enum):
            VERDE = auto()
            AMARELO = auto()

        c: Cor = Cor.VERDE
        res: int = 0
        match c:
            case Cor.VERDE:
                res = 1
            case Cor.AMARELO:
                res = 2
        assert res == 1
    "});
}


