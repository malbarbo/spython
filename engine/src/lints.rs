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
    /// Forbids `if`/`elif`/`else` before level 1 (Selection).
    pub(crate) static FORBIDDEN_SELECTION = {
        summary: "`if` not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

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

declare_lint! {
    /// ## What it does
    /// Checks that conditions in `if` / `elif` / `while` / ternary expressions,
    /// operands of `and` / `or` / `not`, and `assert` tests have type `bool`
    /// at teaching levels 0–3.
    ///
    /// ## Why is this bad?
    /// Python's truthiness rules (non-zero int is truthy, non-empty string is
    /// truthy, etc.) are a common source of bugs for students. At the teaching
    /// levels, requiring explicit boolean expressions forces students to write
    /// the comparison they actually mean.
    ///
    /// ## Example
    /// ```python
    /// x: int = 3
    /// if x:          # error: condition is int, not bool
    ///     pass
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// x: int = 3
    /// if x > 0:
    ///     pass
    /// ```
    pub(crate) static NON_BOOLEAN_CONDITION = {
        summary: "condition is not a boolean value",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects `bool` operands in arithmetic operators (`+`, `-`, `*`, `/`,
    /// `//`, `%`, `**`), augmented assignment with those operators, and
    /// unary `+` / `-` at teaching levels 0–3.
    ///
    /// ## Why is this bad?
    /// Python treats `bool` as a subclass of `int`, so `True + 1` silently
    /// evaluates to `2`. This hides bugs where a student used a boolean
    /// where they meant a number.
    ///
    /// ## Example
    /// ```python
    /// def f(x: int) -> int:
    ///     return x + True   # error: bool operand in arithmetic
    /// ```
    pub(crate) static BOOL_IN_ARITHMETIC = {
        summary: "bool operand in arithmetic expression",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects chained comparisons like `a < b < c` or `a == b != c` at
    /// teaching levels 0–3.
    ///
    /// ## Why is this bad?
    /// Python interprets `a == b != c` as `(a == b) and (b != c)`, which is
    /// surprising for beginners who expect left-to-right evaluation. Requiring
    /// explicit `and` / `or` forces students to write what they mean.
    ///
    /// ## Example
    /// ```python
    /// if a < b < c:          # error: chained comparison
    ///     pass
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// if a < b and b < c:
    ///     pass
    /// ```
    pub(crate) static CHAINED_COMPARISON = {
        summary: "chained comparison",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects expression statements whose result is discarded (other than
    /// function calls and string/ellipsis literals) at teaching levels 0–3.
    ///
    /// ## Why is this bad?
    /// An expression statement like `x + 1` has no effect — the value is
    /// computed and thrown away. It usually means the student forgot an
    /// assignment or a `print`. Calls are allowed because they may have side
    /// effects, and string literals are allowed as docstrings.
    ///
    /// ## Example
    /// ```python
    /// def f(x: int) -> int:
    ///     x + 1        # error: result is discarded
    ///     return x
    /// ```
    pub(crate) static BARE_EXPRESSION = {
        summary: "bare expression statement has no effect",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects default values on function parameters at teaching levels 0–3.
    ///
    /// ## Why is this bad?
    /// Default arguments add subtle behavior — especially mutable defaults
    /// that are shared between calls — and hide which values a function
    /// actually needs. At the teaching levels, students should pass every
    /// argument explicitly.
    ///
    /// ## Example
    /// ```python
    /// def f(x: int = 0) -> int:   # error: default argument
    ///     return x
    /// ```
    pub(crate) static FORBIDDEN_DEFAULT_ARG = {
        summary: "default argument value is not allowed at this level",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that function names follow the `snake_case` naming convention
    /// (lowercase letters, digits, and underscores).
    ///
    /// ## Why is this bad?
    /// PEP 8 recommends `snake_case` for function names. Mixing styles makes
    /// student code harder to read and inconsistent with most Python libraries.
    ///
    /// Methods explicitly decorated with `@override` or `@overload` are
    /// exempt — those are defined elsewhere with the original name.
    ///
    /// ## Example
    /// ```python
    /// def myFunction() -> None:    # error: should be lowercase
    ///     pass
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def my_function() -> None:
    ///     pass
    /// ```
    pub(crate) static INVALID_FUNCTION_NAME = {
        summary: "function name should be lowercase",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that class names follow the `CapWords` (CamelCase) convention.
    ///
    /// ## Why is this bad?
    /// PEP 8 recommends `CapWords` for class names. The check accepts an
    /// optional leading underscore and rejects names containing underscores
    /// after the first character (e.g. `My_Class`) or names starting with a
    /// lowercase letter (e.g. `my_class`).
    ///
    /// ## Example
    /// ```python
    /// class my_class:    # error: should use CapWords
    ///     pass
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// class MyClass:
    ///     pass
    /// ```
    pub(crate) static INVALID_CLASS_NAME = {
        summary: "class name should use CapWords convention",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that function and method parameters use `snake_case` (no
    /// uppercase letters).
    ///
    /// ## Why is this bad?
    /// PEP 8 recommends lowercase parameter names, matching variable and
    /// function naming. Methods explicitly decorated with `@override` are
    /// exempt — the parameter names there mirror the parent class.
    ///
    /// ## Example
    /// ```python
    /// def f(X: int) -> int:    # error: argument should be lowercase
    ///     return X
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def f(x: int) -> int:
    ///     return x
    /// ```
    pub(crate) static INVALID_ARGUMENT_NAME = {
        summary: "argument name should be lowercase",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that variables assigned inside a function (including loop
    /// targets) use `snake_case`. Module-level `UPPER_CASE` constants are
    /// allowed and not flagged.
    ///
    /// ## Why is this bad?
    /// PEP 8 recommends lowercase names for local variables. Uppercase or
    /// mixedCase locals confuse the reader: `UPPER_CASE` is the convention
    /// for module-level constants.
    ///
    /// ## Example
    /// ```python
    /// def f(x: int) -> int:
    ///     Y: int = x + 1    # error: variable should be lowercase
    ///     return Y
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def f(x: int) -> int:
    ///     y: int = x + 1
    ///     return y
    /// ```
    pub(crate) static NON_LOWERCASE_VARIABLE_IN_FUNCTION = {
        summary: "variable in function should be lowercase",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that members of an `Enum` (or `IntEnum`, `StrEnum`, `Flag`,
    /// `IntFlag`) subclass use `UPPER_CASE` names.
    ///
    /// ## Why is this bad?
    /// PEP 8 treats enum members as class-level constants and recommends
    /// `UPPER_CASE_WITH_UNDERSCORES` for them. Methods on the enum class are
    /// not flagged.
    ///
    /// ## Example
    /// ```python
    /// from enum import IntEnum
    ///
    /// class Color(IntEnum):
    ///     red = 1     # error: should be uppercase
    ///     blue = 2
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// from enum import IntEnum
    ///
    /// class Color(IntEnum):
    ///     RED = 1
    ///     BLUE = 2
    /// ```
    pub(crate) static NON_UPPERCASE_ENUM_MEMBER = {
        summary: "enum member should be uppercase",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects equality comparisons against `None` (`x == None`, `x != None`).
    ///
    /// ## Why is this bad?
    /// PEP 8 says comparisons to singletons like `None` should always use
    /// `is` / `is not`, never `==` / `!=`. The two are not always equivalent
    /// (custom `__eq__` can lie), and `is None` is the idiomatic Python.
    pub(crate) static NONE_COMPARISON = {
        summary: "comparison to `None` should use `is` / `is not`",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects equality comparisons against `True` or `False` (e.g.
    /// `x == True`).
    ///
    /// ## Why is this bad?
    /// Comparing a boolean to `True` is redundant — write `x` instead of
    /// `x == True`, and `not x` instead of `x == False`.
    pub(crate) static TRUE_FALSE_COMPARISON = {
        summary: "comparison to `True` / `False` is redundant",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects `not <a> in <b>`; suggests `<a> not in <b>`.
    ///
    /// ## Why is this bad?
    /// The `not in` operator reads more naturally and is what PEP 8
    /// recommends. `not x in y` looks like `(not x) in y` to a careless
    /// reader.
    pub(crate) static NOT_IN_TEST = {
        summary: "test for membership should use `not in`",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects `not <a> is <b>`; suggests `<a> is not <b>`.
    ///
    /// ## Why is this bad?
    /// `is not` reads more naturally than `not ... is ...` and is the form
    /// PEP 8 recommends.
    pub(crate) static NOT_IS_TEST = {
        summary: "test for object identity should use `is not`",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Checks that the first parameter of an instance method is named
    /// `self`. `@staticmethod` and `@classmethod` are skipped.
    ///
    /// ## Why is this bad?
    /// PEP 8 says: "Always use `self` for the first argument to instance
    /// methods." Diverging confuses readers and tools that expect the
    /// convention.
    ///
    /// ## Example
    /// ```python
    /// class Box:
    ///     x: int
    ///     def __init__(this, x: int) -> None:    # error
    ///         this.x = x
    /// ```
    pub(crate) static INVALID_FIRST_ARGUMENT_NAME_FOR_METHOD = {
        summary: "first argument of an instance method should be `self`",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Detects an `if` whose only purpose is to return `True` or `False`,
    /// like `if cond: return True else: return False`.
    ///
    /// ## Why is this bad?
    /// `return cond` (or `return not cond`) says the same thing in one
    /// line. The longer form suggests the author didn't realize a boolean
    /// expression is already a value.
    ///
    /// ## Example
    /// ```python
    /// def positive(x: int) -> bool:
    ///     if x > 0:
    ///         return True
    ///     else:
    ///         return False
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def positive(x: int) -> bool:
    ///     return x > 0
    /// ```
    pub(crate) static NEEDLESS_BOOL = {
        summary: "return the condition directly instead of `if cond: return True else: return False`",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Detects f-strings that have no placeholder expressions
    /// (e.g. `f"hello"`).
    ///
    /// ## Why is this bad?
    /// An f-string without placeholders is just a regular string, and the
    /// `f` prefix is misleading: a reader might expect interpolations that
    /// aren't there. It can also indicate that the author forgot to add
    /// the `{...}` they meant to.
    pub(crate) static F_STRING_MISSING_PLACEHOLDERS = {
        summary: "f-string has no placeholders; use a regular string",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Detects local variables in a function that are assigned to but
    /// never read. Only enabled at teaching levels 0–3, since at higher
    /// levels closures and shadowing make the simple AST-based detection
    /// unreliable.
    ///
    /// Variables whose name starts with `_` are exempt (the conventional
    /// "I know it's unused" marker).
    ///
    /// ## Example
    /// ```python
    /// def f(x: int) -> int:
    ///     y: int = x + 1   # error: y is never read
    ///     return x
    /// ```
    pub(crate) static UNUSED_VARIABLE = {
        summary: "local variable is assigned but never used",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Detects `pass` statements in a body that already contains other
    /// statements (e.g. a docstring followed by `pass`). `pass` is a
    /// placeholder for an empty body and is unnecessary in that case.
    ///
    /// ## Example
    /// ```python
    /// def f() -> None:
    ///     """Docstring."""
    ///     pass    # error: unnecessary
    /// ```
    pub(crate) static UNNECESSARY_PASS = {
        summary: "unnecessary `pass` statement",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}

declare_lint! {
    /// ## What it does
    /// Rejects docstring lines that start with `>>>` or `...` but are not
    /// followed by a single space (e.g. `>>>foo`). The doctest runner and
    /// the stdlib `doctest` module silently ignore such lines, so the test
    /// appears to pass while not running at all.
    ///
    /// ## Example
    /// ```python
    /// def add(x: int, y: int) -> int:
    ///     """
    ///     >>>add(1, 2)    # error: missing space after `>>>`
    ///     3
    ///     """
    ///     return x + y
    /// ```
    ///
    /// Use instead:
    /// ```python
    /// def add(x: int, y: int) -> int:
    ///     """
    ///     >>> add(1, 2)
    ///     3
    ///     """
    ///     return x + y
    /// ```
    pub(crate) static DOCTEST_MALFORMED_PROMPT = {
        summary: "malformed doctest prompt",
        status: LintStatus::preview("0.0.0"),
        default_level: Level::Error,
    }
}
