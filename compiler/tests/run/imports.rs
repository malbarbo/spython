//! The imports of `math`, `enum` and `dataclasses`.

use indoc::{formatdoc, indoc};

use super::{same_as_cpython, unsupported};

#[test]
fn an_import_of_math_runs() {
    same_as_cpython(
        "an_import_of_math_runs",
        indoc! {"
            import math
            import math as m
            from math import floor, ceil as c

            print(1)
        "},
    );
}

#[test]
fn an_import_of_enum_and_of_dataclasses_runs() {
    same_as_cpython(
        "an_import_of_enum_and_of_dataclasses_runs",
        indoc! {"
            import enum
            import dataclasses as dc
            from enum import Enum, auto as a
            from dataclasses import dataclass

            print(1)
        "},
    );
}

/// A name that an import binds is a global, which a `NameError` suggests
/// once the import has run.
#[test]
fn the_name_error_suggests_a_name_after_its_import() {
    for (name, before, after) in [
        (
            "the_name_error_suggests_a_name_after_its_import",
            "import math\n",
            "",
        ),
        (
            "the_name_error_suggests_no_name_before_its_import",
            "",
            "import math\n",
        ),
    ] {
        same_as_cpython(
            name,
            &formatdoc! {"
                {before}
                def f() -> int:
                    return mat

                print(f())
                mat = 1
                {after}"},
        );
    }
}

#[test]
fn an_import_outside_what_the_compiler_covers_has_its_own_message() {
    assert_eq!(
        unsupported(indoc! {"
            import typing
        "}),
        "an import of this module"
    );
    assert_eq!(
        unsupported(indoc! {"
            from typing import Final
        "}),
        "an import of this name"
    );
    assert_eq!(
        unsupported(indoc! {"
            from math import *
        "}),
        "an import of every name"
    );
    assert_eq!(
        unsupported(indoc! {"
            from enum import IntEnum
        "}),
        "an import of this name"
    );
    assert_eq!(
        unsupported(indoc! {"
            from dataclasses import field
        "}),
        "an import of this name"
    );
    assert_eq!(
        unsupported(indoc! {"
            from enum import auto

            print(auto)
        "}),
        "a name of an import as a value"
    );
    assert_eq!(
        unsupported(indoc! {"
            import enum

            print(enum.auto)
        "}),
        "this expression"
    );
    assert_eq!(
        unsupported(indoc! {"
            def f() -> None:
                import math
        "}),
        "an import in a function"
    );
    assert_eq!(
        unsupported(indoc! {"
            import math as self
        "}),
        "an import that binds self"
    );
    assert_eq!(
        unsupported(indoc! {"
            import math

            print(math)
        "}),
        "a name of an import as a value"
    );
    assert_eq!(
        unsupported(indoc! {"
            from math import floor

            def floor() -> None:
                pass
        "}),
        "a name of an import and of a function"
    );
    assert_eq!(
        unsupported(indoc! {"
            from math import floor
            from math import ceil as floor
        "}),
        "a name of two imports"
    );
}
