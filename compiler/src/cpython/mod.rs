//! The rules of CPython 3.13 that the compiler computes in Rust, with the
//! texts that CPython writes. Nothing here knows the IR or wasm.

pub(crate) mod names;
pub(crate) mod suggest;
pub(crate) mod traceback;

/// The most frames CPython keeps, with its default recursion limit. The
/// frame of the module counts.
pub(crate) const RECURSION_LIMIT: i32 = 1000;

/// The traceback writes the first frames of a run of frames on the same
/// line, and then how many more there are.
pub(crate) const RECURSIVE_CUTOFF: i32 = 3;

/// The first line of a traceback.
pub(crate) const TRACEBACK: &str = "Traceback (most recent call last):\n";

/// The line of a traceback for a run of frames past the cutoff is
/// `REPEATED`, the count, `MORE_TIME`, an `s` for more than one, and
/// `REPEATED_END`.
pub(crate) const REPEATED: &str = "  [Previous line repeated ";
pub(crate) const MORE_TIME: &str = " more time";
pub(crate) const REPEATED_END: &str = "]\n";

pub(crate) const MEMORY_ERROR: &str = "MemoryError";
pub(crate) const RECURSION_ERROR: &str = "RecursionError: maximum recursion depth exceeded";
pub(crate) const DIVISION_BY_ZERO: &str = "ZeroDivisionError: integer division or modulo by zero";
pub(crate) const MODULO_BY_ZERO: &str = "ZeroDivisionError: integer modulo by zero";
pub(crate) const STR_INDEX_OUT_OF_RANGE: &str = "IndexError: string index out of range";
