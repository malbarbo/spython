mod testlib;

use indoc::indoc;
use testlib::run;

#[test]
fn aug_assign_add() {
    run(indoc! {"
        x: int = 10
        x += 5
        assert x == 15
    "});
}

#[test]
fn aug_assign_sub() {
    run(indoc! {"
        x: int = 10
        x -= 3
        assert x == 7
    "});
}

#[test]
fn aug_assign_mul() {
    run(indoc! {"
        x: int = 10
        x *= 2
        assert x == 20
    "});
}

#[test]
fn aug_assign_div() {
    run(indoc! {"
        x: float = 10.0
        x /= 2.5
        assert x == 4.0
    "});
}

#[test]
fn while_basic() {
    run(indoc! {"
        x: int = 0
        while x < 5:
            x += 1
        assert x == 5
    "});
}

#[test]
fn while_else() {
    run(indoc! {"
        x: int = 0
        while x < 5:
            x += 1
        else:
            x = 100
        assert x == 100
    "});
}

#[test]
fn list_literal_int() {
    run(indoc! {"
        xs: list[int] = [1, 2, 3]
        assert True
    "});
}

#[test]
fn for_basic() {
    run(indoc! {"
        xs: list[int] = [1, 2, 3]
        sum: int = 0
        for x in xs:
            sum += x
        assert sum == 6
    "});
}

#[test]
fn for_else() {
    run(indoc! {"
        xs: list[int] = [1, 2, 3]
        sum: int = 0
        for x in xs:
            sum += x
        else:
            sum = 100
        assert sum == 100
    "});
}

