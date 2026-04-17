mod testlib;

use indoc::indoc;
use testlib::run;

#[test]
fn chained_cmp_single_eval() {
    run(indoc! {"
        count: int = 0

        def inc() -> int:
            global count
            count = count + 1
            return count

        assert 0 < inc() < 2
        assert count == 1

        count = 0
        assert not (0 < inc() < 0)
        assert count == 1
    "});
}
