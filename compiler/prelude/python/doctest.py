"""The report of the doctests of a file, as the runner of spython writes it."""

from __spython_prelude__.intrinsics import (
    builder_done,
    builder_new,
    captured_exception,
    captured_output,
    code_point,
    write_err,
)
from __spython_prelude__.str import (
    find,
    is_space,
    put,
    put_hex_escape,
    put_range,
    startswith,
)


def start() -> None:
    write_err("Running tests...\n")


def group(name: str) -> None:
    write_err("  " + name + "\n")


def example(
    source: str,
    shown_want: str,
    want: str,
    exc: str,
    expects: bool,
    verbose: bool,
) -> int:
    """Reports the example that has just run, and returns 0 when it passes, 1 when it
    fails and 2 when it raises an exception that it does not expect.

    `source` and `shown_want` are the code and the output as the report shows them.
    `want` is the output that the example asks for, and `exc` is the exception that
    the example expects, when `expects` is true.
    """
    got = captured_output()
    exc_line = captured_exception()
    raised = len(exc_line) > 0
    if not raised:
        ok = same_output(want, got)
    elif expects:
        got = exc_line
        ok = same_output(exc, got)
    else:
        got = got + exc_line
        ok = same_output(want, got)
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


def same_output(want: str, got: str) -> bool:
    """Returns `True` if `got` is the output that `want` asks for, `False` otherwise.
    The two match in ASCII, and a line of `want` that is `<BLANKLINE>` asks for a
    line of spaces."""
    ascii_want = to_ascii(want)
    ascii_got = to_ascii(got)
    if ascii_got == ascii_want:
        return True
    return emptied(ascii_got, False) == emptied(ascii_want, True)


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
            put_hex_escape(b, at, c, k)
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
    end = find(s, "\n", start, len(s))
    if end < 0:
        return len(s)
    return end


def only(s: str, start: int, end: int, spaces: bool) -> bool:
    """Returns `True` if each code point of `s` from `start` to `end` is a
    blank, or a space of any kind when `spaces` is true, `False` otherwise."""
    while start < end:
        c = code_point(s, start)
        if spaces and not is_space(c):
            return False
        if not spaces and c != 32:
            return False
        start += 1
    return True


def emptied(s: str, marks: bool) -> str:
    """The ASCII text `s` with nothing in each line that `empty` accepts."""
    n = len(s)
    size = 0
    i = 0
    while i <= n:
        end = line_end(s, i)
        if not empty(s, i, end, marks):
            size += end - i
        if end < n:
            size += 1
        i = end + 1
    b = builder_new(size)
    at = 0
    i = 0
    while i <= n:
        end = line_end(s, i)
        if not empty(s, i, end, marks):
            at = put_range(b, at, s, i, end)
        if end < n:
            b[at] = 10
            at += 1
        i = end + 1
    return builder_done(b)


def empty(s: str, start: int, end: int, marks: bool) -> bool:
    """Returns `True` if the line of `s` from `start` to `end` has only spaces,
    after a `<BLANKLINE>` when `marks` is true, `False` otherwise."""
    if marks:
        mark = "<BLANKLINE>"
        if not startswith(s, mark, start, end):
            return False
        start += len(mark)
    return only(s, start, end, True)


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
            at = put(b, at, mark)
        else:
            at = put_range(b, at, output, i, end)
        if end + 1 < n:
            at = put(b, at, indent)
        i = end + 1
    return builder_done(b)


def first_line(s: str) -> str:
    return s[: line_end(s, 0)]
