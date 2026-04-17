mod testlib;

use indoc::indoc;
use testlib::run;

#[test]
fn string_indexing() {
    run(indoc! {"
        s: str = 'abc'
        assert s[0] == 97  # 'a' in ASCII/Unicode
        assert s[1] == 98  # 'b'
        assert s[2] == 99  # 'c'
    "});
}

#[test]
fn list_indexing() {
    run(indoc! {"
        xs: list[int] = [10, 20, 30]
        assert xs[0] == 10
        assert xs[1] == 20
        assert xs[2] == 30
    "});
}

#[test]
fn lambda_basic() {
    run(indoc! {"
        def call_it(f: Callable[[int], int], x: int) -> int:
            return f(x)
        
        assert call_it(lambda x: x + 1, 5) == 6
    "});
}

#[test]
fn class_method() {
    run(indoc! {"
        class Counter:
            val: int
            def __init__(self, start: int) -> None:
                self.val = start
            
            def inc(self) -> int:
                self.val += 1
                return self.val

        c: Counter = Counter(10)
        assert c.val == 10
        assert c.inc() == 11
        assert c.val == 11
    "});
}
