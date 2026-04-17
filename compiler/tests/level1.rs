mod testlib;

use indoc::indoc;
use testlib::run;

#[test]
fn if_basic() {
    run(indoc! {"
        x: int = 10
        if x > 5:
            assert True
        else:
            assert False
    "});
}

#[test]
fn if_else() {
    run(indoc! {"
        x: int = 3
        if x > 5:
            assert False
        else:
            assert True
    "});
}

#[test]
fn if_elif_else() {
    run(indoc! {"
        x: int = 5
        if x > 10:
            assert False
        elif x > 0:
            assert True
        else:
            assert False
    "});
}

#[test]
fn if_no_else() {
    run(indoc! {"
        x: int = 10
        if x > 5:
            x = 20
        assert x == 20
    "});
}

#[test]
fn string_len() {
    run(indoc! {"
        s: str = 'abc'
        assert len(s) == 3
        assert len('') == 0
    "});
}

