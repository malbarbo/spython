//! The rules of CPython 3.13 that the compiler computes in Rust, with the
//! texts that CPython writes. Nothing here knows the IR or wasm.

pub(crate) mod exception;
pub(crate) mod names;
pub(crate) mod suggest;
pub(crate) mod traceback;

pub(crate) use exception::{Class, Message};

/// The most frames CPython keeps, with its default recursion limit. The
/// frame of the module counts.
pub(crate) const RECURSION_LIMIT: i32 = 1000;

/// The traceback writes the first frames of a run of frames on the same
/// line, and then how many more there are.
pub(crate) const RECURSIVE_CUTOFF: i32 = 3;

/// The first line of a traceback.
pub(crate) const TRACEBACK: &str = "Traceback (most recent call last):\n";

/// The line of a traceback for a run of frames past the cutoff is
/// `REPEATED`, the count, and `MORE_TIME` for one frame or `MORE_TIMES`
/// for more.
pub(crate) const REPEATED: &str = "  [Previous line repeated ";
pub(crate) const MORE_TIME: &str = " more time]\n";
pub(crate) const MORE_TIMES: &str = " more times]\n";

/// The line before the error of the flush of `sys.stdout` at the exit.
pub(crate) const FLUSH_FAILED: &str = "Exception ignored on flushing sys.stdout:\n";
/// The exit code of CPython when the flush at the exit fails.
pub(crate) const FLUSH_FAILED_EXIT: i32 = 120;

pub(crate) const MEMORY_ERROR: Message = Message::new(Class::MemoryError, "");
pub(crate) const RECURSION_ERROR: Message =
    Message::new(Class::RecursionError, "maximum recursion depth exceeded");
pub(crate) const DIVISION_BY_ZERO: Message = Message::new(
    Class::ZeroDivisionError,
    "integer division or modulo by zero",
);
pub(crate) const MODULO_BY_ZERO: Message =
    Message::new(Class::ZeroDivisionError, "integer modulo by zero");
pub(crate) const INT_TRUE_DIVISION_BY_ZERO: Message =
    Message::new(Class::ZeroDivisionError, "division by zero");
pub(crate) const FLOAT_TRUE_DIVISION_BY_ZERO: Message =
    Message::new(Class::ZeroDivisionError, "float division by zero");
pub(crate) const FLOAT_FLOOR_DIVISION_BY_ZERO: Message =
    Message::new(Class::ZeroDivisionError, "float floor division by zero");
pub(crate) const FLOAT_MODULO_BY_ZERO: Message =
    Message::new(Class::ZeroDivisionError, "float modulo by zero");
pub(crate) const ZERO_TO_A_NEGATIVE_POWER: Message = Message::new(
    Class::ZeroDivisionError,
    "0.0 cannot be raised to a negative power",
);
pub(crate) const MATH_DOMAIN_ERROR: Message = Message::new(Class::ValueError, "math domain error");
pub(crate) const MATH_RANGE_ERROR: Message = Message::new(Class::OverflowError, "math range error");
pub(crate) const FMA_INVALID: Message = Message::new(Class::ValueError, "invalid operation in fma");
pub(crate) const FMA_OVERFLOW: Message = Message::new(Class::OverflowError, "overflow in fma");
pub(crate) const NEGATIVE_TOLERANCE: Message =
    Message::new(Class::ValueError, "tolerances must be non-negative");
pub(crate) const NEGATIVE_N: Message =
    Message::new(Class::ValueError, "n must be a non-negative integer");
pub(crate) const NEGATIVE_K: Message =
    Message::new(Class::ValueError, "k must be a non-negative integer");
pub(crate) const ROUNDED_TOO_LARGE: Message =
    Message::new(Class::OverflowError, "rounded value too large to represent");
/// The `OverflowError` of `errno` `ERANGE`, with the `strerror` of glibc.
pub(crate) const RESULT_OUT_OF_RANGE: Message = Message::new(
    Class::OverflowError,
    "(34, 'Numerical result out of range')",
);
pub(crate) const NAN_TO_INT: Message =
    Message::new(Class::ValueError, "cannot convert float NaN to integer");
pub(crate) const INFINITY_TO_INT: Message = Message::new(
    Class::OverflowError,
    "cannot convert float infinity to integer",
);
pub(crate) const STR_INDEX_OUT_OF_RANGE: Message =
    Message::new(Class::IndexError, "string index out of range");

/// The last line of the traceback of the `OSError` that CPython raises on
/// Linux for each error of WASI that wasmtime can give to `fd_write`. These
/// are the errors of the `error-code` of `wasi:filesystem`.
pub(crate) const OS_ERRORS: [(i32, Class, &str); 37] = [
    (2, Class::PermissionError, "[Errno 13] Permission denied"),
    (
        6,
        Class::BlockingIOError,
        "[Errno 11] Resource temporarily unavailable",
    ),
    (
        7,
        Class::BlockingIOError,
        "[Errno 114] Operation already in progress",
    ),
    (8, Class::OSError, "[Errno 9] Bad file descriptor"),
    (10, Class::OSError, "[Errno 16] Device or resource busy"),
    (16, Class::OSError, "[Errno 35] Resource deadlock avoided"),
    (19, Class::OSError, "[Errno 122] Disk quota exceeded"),
    (20, Class::FileExistsError, "[Errno 17] File exists"),
    (22, Class::OSError, "[Errno 27] File too large"),
    (
        25,
        Class::OSError,
        "[Errno 84] Invalid or incomplete multibyte or wide character",
    ),
    (
        26,
        Class::BlockingIOError,
        "[Errno 115] Operation now in progress",
    ),
    (
        27,
        Class::InterruptedError,
        "[Errno 4] Interrupted system call",
    ),
    (28, Class::OSError, "[Errno 22] Invalid argument"),
    (29, Class::OSError, "[Errno 5] Input/output error"),
    (31, Class::IsADirectoryError, "[Errno 21] Is a directory"),
    (
        32,
        Class::OSError,
        "[Errno 40] Too many levels of symbolic links",
    ),
    (34, Class::OSError, "[Errno 31] Too many links"),
    (35, Class::OSError, "[Errno 90] Message too long"),
    (37, Class::OSError, "[Errno 36] File name too long"),
    (43, Class::OSError, "[Errno 19] No such device"),
    (
        44,
        Class::FileNotFoundError,
        "[Errno 2] No such file or directory",
    ),
    (46, Class::OSError, "[Errno 37] No locks available"),
    (48, Class::OSError, "[Errno 12] Cannot allocate memory"),
    (51, Class::OSError, "[Errno 28] No space left on device"),
    (54, Class::NotADirectoryError, "[Errno 20] Not a directory"),
    (55, Class::OSError, "[Errno 39] Directory not empty"),
    (56, Class::OSError, "[Errno 131] State not recoverable"),
    (58, Class::OSError, "[Errno 95] Operation not supported"),
    (
        59,
        Class::OSError,
        "[Errno 25] Inappropriate ioctl for device",
    ),
    (60, Class::OSError, "[Errno 6] No such device or address"),
    (
        61,
        Class::OSError,
        "[Errno 75] Value too large for defined data type",
    ),
    (
        63,
        Class::PermissionError,
        "[Errno 1] Operation not permitted",
    ),
    (64, Class::BrokenPipeError, "[Errno 32] Broken pipe"),
    (69, Class::OSError, "[Errno 30] Read-only file system"),
    (70, Class::OSError, "[Errno 29] Illegal seek"),
    (74, Class::OSError, "[Errno 26] Text file busy"),
    (75, Class::OSError, "[Errno 18] Invalid cross-device link"),
];
