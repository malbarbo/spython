"""The report of the doctests of a file, as the runner of spython writes it."""

from __spython_prelude__.intrinsics import (
    StrBuilder,
    builder_done,
    builder_new,
    captured_exception,
    captured_output,
    code_point,
    write_err,
)
from __spython_prelude__.str import hex_digit


def start() -> None:
    write_err("Running tests...\n")


def group(name: str) -> None:
    write_err("  " + name + "\n")


def example(
    source: str,
    shown_want: str,
    want: str,
    blank_want: str,
    exc: str,
    blank_exc: str,
    expects: bool,
    verbose: bool,
) -> int:
    """Reports the example that has just run, and returns 0 when it passes, 1 when it
    fails and 2 when it raises an exception that it does not expect.

    `source` and `shown_want` are the code and the output as the report shows them.
    `want` is the output in ASCII, and `blank_want` is `want` with nothing in the
    place of each `<BLANKLINE>`. `exc` and `blank_exc` are the same for the exception
    that the example expects, when `expects` is true.
    """
    got = captured_output()
    exc_line = captured_exception()
    raised = len(exc_line) > 0
    if not raised:
        ok = same_output(want, blank_want, got)
    elif expects:
        got = exc_line
        ok = same_output(exc, blank_exc, got)
    else:
        got = got + exc_line
        ok = same_output(want, blank_want, got)
    if ok:
        if verbose:
            write_err("    ok: " + first_line(source) + "\n")
        return 0
    if raised and not expects:
        write_err(
            "    Error in example:\n      "
            + source
            + "\n    Got:\n      "
            + shown(got)
            + "\n"
        )
        return 2
    write_err(
        "    Failed example:\n      "
        + source
        + "\n    Expected:\n      "
        + shown_want
        + "\n    Got:\n      "
        + shown(got)
        + "\n"
    )
    return 1


def summary(total: int, failed: int, errors: int) -> int:
    """Writes the last line of the report, and returns the number of examples that
    did not pass."""
    write_err(
        plural(total, "test")
        + ", "
        + plural(total - failed - errors, "success")
        + ", "
        + plural(failed, "failure")
        + " and "
        + plural(errors, "error")
        + ".\n"
    )
    return failed + errors


def plural(n: int, word: str) -> str:
    if n == 1:
        return "1 " + word
    if word == "success":
        return str(n) + " " + word + "es"
    return str(n) + " " + word + "s"


def same_output(want: str, blank_want: str, got: str) -> bool:
    """Returns `True` if `got` is the output that `want` asks for, `False` otherwise."""
    ascii_got = to_ascii(got)
    if ascii_got == want:
        return True
    return blank_spaces(ascii_got) == blank_want


def to_ascii(s: str) -> str:
    """`s` with an escape in the place of each code point that is not ASCII."""
    n = len(s)
    size = 0
    i = 0
    while i < n:
        size += ascii_len(code_point(s, i))
        i += 1
    if size == n:
        return s
    b = builder_new(size)
    at = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = ascii_len(c)
        if k == 1:
            b[at] = c
        else:
            b[at] = 92
            if k == 4:
                b[at + 1] = 120
            elif k == 6:
                b[at + 1] = 117
            else:
                b[at + 1] = 85
            j = k - 1
            while j >= 2:
                b[at + j] = hex_digit(c % 16)
                c = c // 16
                j -= 1
        at += k
        i += 1
    return builder_done(b)


def ascii_len(c: int) -> int:
    if c < 128:
        return 1
    if c < 256:
        return 4
    if c < 65536:
        return 6
    return 10


def line_end(s: str, start: int) -> int:
    """The index of the first line feed of `s` from `start`, or the length of `s`."""
    n = len(s)
    while start < n and code_point(s, start) != 10:
        start += 1
    return start


def is_space(c: int) -> bool:
    """Whether `str.isspace` accepts the ASCII code point `c`."""
    return c == 32 or (9 <= c and c <= 13) or (28 <= c and c <= 31)


def only(s: str, start: int, end: int, spaces: bool) -> bool:
    """Whether each code point of `s` from `start` to `end` is a blank, or a space
    of any kind when `spaces` is true."""
    while start < end:
        c = code_point(s, start)
        if spaces and not is_space(c):
            return False
        if not spaces and c != 32:
            return False
        start += 1
    return True


def blank_spaces(s: str) -> str:
    """The ASCII text `s` without the spaces of each line that has nothing else."""
    n = len(s)
    size = 0
    i = 0
    while i <= n:
        end = line_end(s, i)
        if not only(s, i, end, True):
            size += end - i
        if end < n:
            size += 1
        i = end + 1
    b = builder_new(size)
    at = 0
    i = 0
    while i <= n:
        end = line_end(s, i)
        if not only(s, i, end, True):
            at = copy(b, at, s, i, end)
        if end < n:
            b[at] = 10
            at += 1
        i = end + 1
    return builder_done(b)


def copy(b: StrBuilder, at: int, s: str, start: int, end: int) -> int:
    """Copies `s` from `start` to `end` into `b` at `at`, and returns the index after
    the copy."""
    while start < end:
        b[at] = code_point(s, start)
        at += 1
        start += 1
    return at


def shown(output: str) -> str:
    """`output` as the report shows it, with `<BLANKLINE>` for a blank line that
    would not show, and each line after the first under the first one."""
    n = len(output)
    mark = "<BLANKLINE>"
    indent = "\n      "
    size = 0
    i = 0
    while i <= n:
        end = line_end(output, i)
        if end < n and only(output, i, end, False):
            size += len(mark)
        else:
            size += end - i
        # The report drops the line feed that ends the output.
        if end + 1 < n:
            size += len(indent)
        i = end + 1
    b = builder_new(size)
    at = 0
    i = 0
    while i <= n:
        end = line_end(output, i)
        if end < n and only(output, i, end, False):
            at = copy(b, at, mark, 0, len(mark))
        else:
            at = copy(b, at, output, i, end)
        if end + 1 < n:
            at = copy(b, at, indent, 0, len(indent))
        i = end + 1
    return builder_done(b)


def first_line(s: str) -> str:
    end = line_end(s, 0)
    b = builder_new(end)
    copy(b, 0, s, 0, end)
    return builder_done(b)
