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

/// The line before the error of the flush of `sys.stdout` at the exit.
pub(crate) const FLUSH_FAILED: &str = "Exception ignored on flushing sys.stdout:\n";
/// The exit code of CPython when the flush at the exit fails.
pub(crate) const FLUSH_FAILED_EXIT: i32 = 120;

pub(crate) const MEMORY_ERROR: &str = "MemoryError";
pub(crate) const RECURSION_ERROR: &str = "RecursionError: maximum recursion depth exceeded";
pub(crate) const DIVISION_BY_ZERO: &str = "ZeroDivisionError: integer division or modulo by zero";
pub(crate) const MODULO_BY_ZERO: &str = "ZeroDivisionError: integer modulo by zero";
pub(crate) const STR_INDEX_OUT_OF_RANGE: &str = "IndexError: string index out of range";

/// The last line of the traceback of the `OSError` that CPython raises on
/// Linux for each error of WASI that wasmtime can give to `fd_write`. These
/// are the errors of the `error-code` of `wasi:filesystem`.
pub(crate) const OS_ERRORS: [(i32, &str); 37] = [
    (2, "PermissionError: [Errno 13] Permission denied"),
    (
        6,
        "BlockingIOError: [Errno 11] Resource temporarily unavailable",
    ),
    (
        7,
        "BlockingIOError: [Errno 114] Operation already in progress",
    ),
    (8, "OSError: [Errno 9] Bad file descriptor"),
    (10, "OSError: [Errno 16] Device or resource busy"),
    (16, "OSError: [Errno 35] Resource deadlock avoided"),
    (19, "OSError: [Errno 122] Disk quota exceeded"),
    (20, "FileExistsError: [Errno 17] File exists"),
    (22, "OSError: [Errno 27] File too large"),
    (
        25,
        "OSError: [Errno 84] Invalid or incomplete multibyte or wide character",
    ),
    (26, "BlockingIOError: [Errno 115] Operation now in progress"),
    (27, "InterruptedError: [Errno 4] Interrupted system call"),
    (28, "OSError: [Errno 22] Invalid argument"),
    (29, "OSError: [Errno 5] Input/output error"),
    (31, "IsADirectoryError: [Errno 21] Is a directory"),
    (32, "OSError: [Errno 40] Too many levels of symbolic links"),
    (34, "OSError: [Errno 31] Too many links"),
    (35, "OSError: [Errno 90] Message too long"),
    (37, "OSError: [Errno 36] File name too long"),
    (43, "OSError: [Errno 19] No such device"),
    (44, "FileNotFoundError: [Errno 2] No such file or directory"),
    (46, "OSError: [Errno 37] No locks available"),
    (48, "OSError: [Errno 12] Cannot allocate memory"),
    (51, "OSError: [Errno 28] No space left on device"),
    (54, "NotADirectoryError: [Errno 20] Not a directory"),
    (55, "OSError: [Errno 39] Directory not empty"),
    (56, "OSError: [Errno 131] State not recoverable"),
    (58, "OSError: [Errno 95] Operation not supported"),
    (59, "OSError: [Errno 25] Inappropriate ioctl for device"),
    (60, "OSError: [Errno 6] No such device or address"),
    (
        61,
        "OSError: [Errno 75] Value too large for defined data type",
    ),
    (63, "PermissionError: [Errno 1] Operation not permitted"),
    (64, "BrokenPipeError: [Errno 32] Broken pipe"),
    (69, "OSError: [Errno 30] Read-only file system"),
    (70, "OSError: [Errno 29] Illegal seek"),
    (74, "OSError: [Errno 26] Text file busy"),
    (75, "OSError: [Errno 18] Invalid cross-device link"),
];
