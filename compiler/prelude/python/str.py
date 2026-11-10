"""The functions of `str`."""

from __spython_prelude__.intrinsics import (
    builder_done,
    builder_new,
    code_point,
    is_printable,
)


def repr_of(s: str) -> str:
    n = len(s)
    single = False
    double = False
    i = 0
    while i < n:
        c = code_point(s, i)
        if c == 39:
            single = True
        elif c == 34:
            double = True
        i += 1
    # The quotes are single unless the text has a single one and no double
    # one.
    quote = 39
    if single and not double:
        quote = 34
    size = 2
    i = 0
    while i < n:
        size += escaped_len(code_point(s, i), quote)
        i += 1
    b = builder_new(size)
    b[0] = quote
    at = 1
    i = 0
    while i < n:
        c = code_point(s, i)
        k = escaped_len(c, quote)
        if k == 1:
            b[at] = c
        else:
            b[at] = 92
            if k == 2:
                b[at + 1] = escape_letter(c)
            else:
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
    b[at] = quote
    return builder_done(b)


def escaped_len(c: int, quote: int) -> int:
    """How many code points `repr` writes for `c` between the quotes."""
    if c == quote or c == 92:
        return 2
    if c == 9 or c == 10 or c == 13:
        return 2
    if c < 32 or c == 127:
        return 4
    if c < 127 or is_printable(c):
        return 1
    if c < 256:
        return 4
    if c < 65536:
        return 6
    return 10


def escape_letter(c: int) -> int:
    """The code point after the backslash of the escape of two code points of `c`."""
    if c == 9:
        return 116
    if c == 10:
        return 110
    if c == 13:
        return 114
    return c


def hex_digit(d: int) -> int:
    if d < 10:
        return 48 + d
    return 87 + d
