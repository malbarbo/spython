"""The functions of `str`."""

from __spython_prelude__.intrinsics import (
    builder_done,
    builder_new,
    code_point,
    is_case_ignorable,
    is_cased,
    is_printable,
    lower_at,
    upper_at,
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


def upper(s: str) -> str:
    n = len(s)
    size = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = 1
        while upper_at(c, k) >= 0:
            k += 1
        size += k
        i += 1
    b = builder_new(size)
    at = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = 0
        m = upper_at(c, 0)
        while m >= 0:
            b[at] = m
            at += 1
            k += 1
            m = upper_at(c, k)
        i += 1
    return builder_done(b)


def lower(s: str) -> str:
    n = len(s)
    size = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = 1
        while lower_at(c, k) >= 0:
            k += 1
        size += k
        i += 1
    b = builder_new(size)
    at = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        if c == 931:
            b[at] = sigma(s, i)
            at += 1
        else:
            k = 0
            m = lower_at(c, 0)
            while m >= 0:
                b[at] = m
                at += 1
                k += 1
                m = lower_at(c, k)
        i += 1
    return builder_done(b)


def find(s: str, sub: str, start: int | None = None, end: int | None = None) -> int:
    n = len(s)
    first = slice_start(start, n)
    last = slice_end(end, n) - len(sub)
    i = first
    while i <= last:
        if found_at(s, sub, i):
            return i
        i += 1
    return -1


def rfind(s: str, sub: str, start: int | None = None, end: int | None = None) -> int:
    n = len(s)
    first = slice_start(start, n)
    i = slice_end(end, n) - len(sub)
    while i >= first:
        if found_at(s, sub, i):
            return i
        i -= 1
    return -1


def index(s: str, sub: str, start: int | None = None, end: int | None = None) -> int:
    i = find(s, sub, start, end)
    if i < 0:
        raise ValueError("substring not found")
    return i


def rindex(s: str, sub: str, start: int | None = None, end: int | None = None) -> int:
    i = rfind(s, sub, start, end)
    if i < 0:
        raise ValueError("substring not found")
    return i


def count(s: str, sub: str, start: int | None = None, end: int | None = None) -> int:
    n = len(s)
    first = slice_start(start, n)
    last = slice_end(end, n)
    m = len(sub)
    if last - first < m:
        return 0
    if m == 0:
        return last - first + 1
    total = 0
    i = first
    while i <= last - m:
        if found_at(s, sub, i):
            total += 1
            i += m
        else:
            i += 1
    return total


def startswith(
    s: str, prefix: str, start: int | None = None, end: int | None = None
) -> bool:
    n = len(s)
    first = slice_start(start, n)
    last = slice_end(end, n) - len(prefix)
    if last < first:
        return False
    return found_at(s, prefix, first)


def endswith(
    s: str, suffix: str, start: int | None = None, end: int | None = None
) -> bool:
    n = len(s)
    first = slice_start(start, n)
    last = slice_end(end, n) - len(suffix)
    if last < first:
        return False
    return found_at(s, suffix, last)


def found_at(s: str, sub: str, i: int) -> bool:
    """Whether `sub` is in `s` from `i` on, with `i + len(sub) <= len(s)`."""
    m = len(sub)
    j = 0
    while j < m:
        if code_point(s, i + j) != code_point(sub, j):
            return False
        j += 1
    return True


def slice_start(start: int | None, n: int) -> int:
    """The first index of a slice of a `str` of length `n`, as CPython adjusts it."""
    if start is None:
        return 0
    if start < 0:
        return max(start + n, 0)
    return start


def slice_end(end: int | None, n: int) -> int:
    """The index after the last of a slice of a `str` of length `n`, as CPython adjusts it."""
    if end is None or end > n:
        return n
    if end < 0:
        return max(end + n, 0)
    return end


def sigma(s: str, i: int) -> int:
    """The small sigma of the capital one at `i` of `s`. The final sigma comes after a cased code
    point and before none, with case ignorable code points between them."""
    j = i - 1
    while j >= 0 and is_case_ignorable(code_point(s, j)):
        j -= 1
    if j < 0 or not is_cased(code_point(s, j)):
        return 963
    n = len(s)
    j = i + 1
    while j < n and is_case_ignorable(code_point(s, j)):
        j += 1
    if j < n and is_cased(code_point(s, j)):
        return 963
    return 962


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
