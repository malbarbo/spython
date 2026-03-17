//! Custom lint rules for spython.
//!
//! This module defines lint rules specific to spython's educational focus:
//! annotation requirements and construct restriction levels.

use ty_python_semantic::declare_lint;
use ty_python_semantic::lint::{Level, LintStatus};

declare_lint! {
    /// ## What it does
    /// Checks that every function parameter (other than the implicit receiver
    /// `self` / `cls` of a method) carries a type annotation.
    ///
    /// ## Why is this bad?
    /// Unannotated parameters prevent the type checker from verifying call sites
    /// and make it harder for readers to understand what types a function expects.
    ///
    /// ## Example
    /// ```python
    /// def add(x, y):       # error: x and y are unannotated
    ///     return x + y
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def add(x: int, y: int) -> int:
    ///     return x + y
    /// ```
    pub(crate) static MISSING_PARAMETER_ANNOTATION = {
        summary: "missing type annotation for a function parameter",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that every function and method declares a return type annotation.
    ///
    /// ## Why is this bad?
    /// Without a return type the type checker cannot verify that callers use the
    /// return value correctly, and the intent of the function is harder to understand.
    ///
    /// ## Example
    /// ```python
    /// def greet(name: str):   # error: no return type
    ///     print(f"Hello, {name}!")
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def greet(name: str) -> None:
    ///     print(f"Hello, {name}!")
    /// ```
    pub(crate) static MISSING_RETURN_ANNOTATION = {
        summary: "missing return type annotation for a function",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that every variable assigned directly in a class body carries
    /// a type annotation.
    ///
    /// ## Why is this bad?
    /// Unannotated class-level assignments are treated as having type `Unknown`
    /// by the type checker, which weakens its ability to reason about the class
    /// and its instances.
    ///
    /// ## Example
    /// ```python
    /// class Point:
    ///     x = 0   # error: no annotation
    ///     y = 0   # error: no annotation
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// class Point:
    ///     x: int = 0
    ///     y: int = 0
    /// ```
    pub(crate) static MISSING_ATTRIBUTE_ANNOTATION = {
        summary: "missing type annotation for a class-body variable assignment",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

// --- Construct restriction lints ---

declare_lint! {
    /// Forbids `for` and `while` loops before level 3.
    pub(crate) static FORBIDDEN_LOOP = {
        summary: "loop not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids list/tuple/dict/set literals before the appropriate level.
    pub(crate) static FORBIDDEN_COLLECTION_LITERAL = {
        summary: "collection literal not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids `class` definitions before level 2.
    pub(crate) static FORBIDDEN_CLASS = {
        summary: "`class` not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids methods inside classes before level 4.
    pub(crate) static FORBIDDEN_CLASS_METHOD = {
        summary: "method in class not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids list/set/dict comprehensions and generator expressions before level 4.
    pub(crate) static FORBIDDEN_COMPREHENSION = {
        summary: "comprehension not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids `lambda` expressions before level 4.
    pub(crate) static FORBIDDEN_LAMBDA = {
        summary: "`lambda` not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids augmented assignment (`+=`, `-=`, etc.) before level 3.
    pub(crate) static FORBIDDEN_AUG_ASSIGN = {
        summary: "augmented assignment not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids constructs like `global`, `nonlocal`, `with`, `try`, `async`,
    /// `yield`, `del` at all teaching levels.
    pub(crate) static FORBIDDEN_CONSTRUCT = {
        summary: "construct not allowed",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// Forbids `match` statements before level 2.
    pub(crate) static FORBIDDEN_MATCH = {
        summary: "`match` not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}
