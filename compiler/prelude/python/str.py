"""The functions of `str`."""

from __spython_prelude__.intrinsics import (
    StrBuilder,
    builder_done,
    builder_new,
    code_point,
    fold_at,
    is_alpha,
    is_case_ignorable,
    is_cased,
    is_decimal,
    is_digit,
    is_lowercase,
    is_numeric,
    is_printable,
    is_uppercase,
    is_xid_continue,
    is_xid_start,
    item,
    lower_at,
    stop,
    title_at,
    upper_at,
)
from __spython_prelude__.slices import (
    slice_end,
    slice_start,
    step_count,
    step_of,
    step_start,
)


def repr_of(s: str) -> str:
    n = len(s)
    singles = 0
    double = False
    # The size without the escapes of the quotes, which the length of a
    # quote leaves out with a quote of -1, which no code point is.
    size = 2
    i = 0
    while i < n:
        c = code_point(s, i)
        if c == 39:
            singles += 1
        elif c == 34:
            double = True
        size += escaped_len(c, -1)
        i += 1
    # The quotes are single unless the text has a single one and no double
    # one. A single quote escapes each single quote of the text.
    quote = 39
    if singles > 0 and not double:
        quote = 34
    else:
        size += singles
    b = builder_new(size)
    b[0] = quote
    at = 1
    i = 0
    while i < n:
        c = code_point(s, i)
        k = escaped_len(c, quote)
        if k == 1:
            b[at] = c
        elif k == 2:
            b[at] = 92
            b[at + 1] = escape_letter(c)
        else:
            put_hex_escape(b, at, c, k)
        at += k
        i += 1
    b[at] = quote
    return builder_done(b)


def repr_or_none(s: str | None) -> str:
    if s is None:
        return "None"
    return repr_of(s)


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


def replace(s: str, old: str, new: str, count: int | None = None) -> str:
    n = len(s)
    m = len(old)
    # An empty `old` is before each code point and at the end.
    limit = n + 1
    if count is not None and count >= 0:
        limit = count
    k = 0
    if m == 0:
        k = min(limit, n + 1)
    else:
        i = 0
        while k < limit and i <= n - m:
            if found_at(s, old, i):
                k += 1
                i += m
            else:
                i += 1
    if k == 0:
        return s
    b = builder_new(n + k * (len(new) - m))
    at = 0
    done = 0
    i = 0
    while i < n:
        if done < k and found_at_most(s, old, i):
            at = put(b, at, new)
            done += 1
            if m > 0:
                i += m
                continue
        b[at] = code_point(s, i)
        at += 1
        i += 1
    if done < k:
        put(b, at, new)
    return builder_done(b)


def found_at_most(s: str, sub: str, i: int) -> bool:
    """Returns `True` if `sub` is in `s` from `i` on, `False` otherwise."""
    return i + len(sub) <= len(s) and found_at(s, sub, i)


def split(s: str) -> list[str]:
    """`s.split()`, the parts of `s` between the runs of white space of `str.isspace`."""
    parts: list[str] = []
    n = len(s)
    i = 0
    while i < n:
        while i < n and is_space(code_point(s, i)):
            i += 1
        j = i
        while j < n and not is_space(code_point(s, j)):
            j += 1
        if i < j:
            parts.append(s[i:j])
        i = j
    return parts


def join(s: str, parts: list[str]) -> str:
    n = len(parts)
    if n == 0:
        return ""
    size = len(s) * (n - 1)
    i = 0
    while i < n:
        size += len(item(parts, i))
        i += 1
    b = builder_new(size)
    at = put(b, 0, item(parts, 0))
    i = 1
    while i < n:
        at = put(b, at, s)
        at = put(b, at, item(parts, i))
        i += 1
    return builder_done(b)


def put(b: StrBuilder, at: int, t: str) -> int:
    """Writes `t` to `b` from `at` on, and gives the index after it."""
    return put_range(b, at, t, 0, len(t))


def put_range(b: StrBuilder, at: int, t: str, start: int, end: int) -> int:
    """Writes `t[start:end]` to `b` from `at` on, and gives the index after it."""
    i = start
    while i < end:
        b[at] = code_point(t, i)
        at += 1
        i += 1
    return at


def removeprefix(s: str, prefix: str) -> str:
    if startswith(s, prefix, None, None):
        return s[len(prefix) :]
    return s


def removesuffix(s: str, suffix: str) -> str:
    if endswith(s, suffix, None, None):
        return s[: len(s) - len(suffix)]
    return s


def strip(s: str, chars: str | None = None) -> str:
    first = strip_start(s, chars)
    return s[first : strip_end(s, chars, first)]


def lstrip(s: str, chars: str | None = None) -> str:
    return s[strip_start(s, chars) :]


def rstrip(s: str, chars: str | None = None) -> str:
    return s[: strip_end(s, chars, 0)]


def strip_start(s: str, chars: str | None) -> int:
    """The index of the first code point of `s` that the strip keeps."""
    n = len(s)
    i = 0
    while i < n and stripped(code_point(s, i), chars):
        i += 1
    return i


def strip_end(s: str, chars: str | None, first: int) -> int:
    """The index after the last code point of `s` from `first` on that the strip keeps."""
    i = len(s)
    while i > first and stripped(code_point(s, i - 1), chars):
        i -= 1
    return i


def stripped(c: int, chars: str | None) -> bool:
    """Returns `True` if a strip of `chars` drops `c`, `False` otherwise. A
    strip of `None` drops the white space."""
    if chars is None:
        return is_space(c)
    n = len(chars)
    i = 0
    while i < n:
        if code_point(chars, i) == c:
            return True
        i += 1
    return False


# Each predicate has a loop of its own, so a program carries the table of the
# predicates that it calls alone.


def isalpha(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_alpha(code_point(s, i)):
            return False
        i += 1
    return n > 0


def isdecimal(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_decimal(code_point(s, i)):
            return False
        i += 1
    return n > 0


def isdigit(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_digit(code_point(s, i)):
            return False
        i += 1
    return n > 0


def isnumeric(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_numeric(code_point(s, i)):
            return False
        i += 1
    return n > 0


def isalnum(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        c = code_point(s, i)
        if not (is_alpha(c) or is_decimal(c) or is_digit(c) or is_numeric(c)):
            return False
        i += 1
    return n > 0


def isspace(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_space(code_point(s, i)):
            return False
        i += 1
    return n > 0


def isascii(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if code_point(s, i) >= 128:
            return False
        i += 1
    return True


def isprintable(s: str) -> bool:
    n = len(s)
    i = 0
    while i < n:
        if not is_printable(code_point(s, i)):
            return False
        i += 1
    return True


def isidentifier(s: str) -> bool:
    n = len(s)
    if n == 0:
        return False
    c = code_point(s, 0)
    if c != 95 and not is_xid_start(c):
        return False
    i = 1
    while i < n:
        if not is_xid_continue(code_point(s, i)):
            return False
        i += 1
    return True


def islower(s: str) -> bool:
    n = len(s)
    cased = False
    i = 0
    while i < n:
        c = code_point(s, i)
        if is_uppercase(c) or is_titlecase(c):
            return False
        if is_lowercase(c):
            cased = True
        i += 1
    return cased


def isupper(s: str) -> bool:
    n = len(s)
    cased = False
    i = 0
    while i < n:
        c = code_point(s, i)
        if is_lowercase(c) or is_titlecase(c):
            return False
        if is_uppercase(c):
            cased = True
        i += 1
    return cased


def istitle(s: str) -> bool:
    """Returns `True` if each cased word of `s` starts with an upper or title
    case code point and goes on in lower case, with at least one word,
    `False` otherwise."""
    n = len(s)
    cased = False
    previous_is_cased = False
    i = 0
    while i < n:
        c = code_point(s, i)
        if is_uppercase(c) or is_titlecase(c):
            if previous_is_cased:
                return False
            previous_is_cased = True
            cased = True
        elif is_lowercase(c):
            if not previous_is_cased:
                return False
            previous_is_cased = True
            cased = True
        else:
            previous_is_cased = False
        i += 1
    return cased


def is_titlecase(c: int) -> bool:
    """Returns `True` if `c` is in the category `Lt`, `False` otherwise."""
    if c < 0x1C5 or c > 0x1FFC:
        return False
    if c < 0x1F88:
        return c == 0x1C5 or c == 0x1C8 or c == 0x1CB or c == 0x1F2
    return (
        c <= 0x1F8F
        or 0x1F98 <= c <= 0x1F9F
        or 0x1FA8 <= c <= 0x1FAF
        or c == 0x1FBC
        or c == 0x1FCC
        or c == 0x1FFC
    )


def is_space(c: int) -> bool:
    """Returns `True` if `str.isspace` accepts `c`, `False` otherwise."""
    if c < 128:
        return c == 32 or 9 <= c <= 13 or 28 <= c <= 31
    if c < 0x2000:
        return c == 0x85 or c == 0xA0 or c == 0x1680
    return (
        c <= 0x200A
        or c == 0x2028
        or c == 0x2029
        or c == 0x202F
        or c == 0x205F
        or c == 0x3000
    )


def center(s: str, width: int, fillchar: str | None = None) -> str:
    fill = fill_code_point(fillchar)
    margin = width - len(s)
    left = margin // 2
    # CPython adds `margin & width & 1`.
    if margin % 2 == 1 and width % 2 == 1:
        left += 1
    return pad(s, left, margin - left, fill)


def ljust(s: str, width: int, fillchar: str | None = None) -> str:
    fill = fill_code_point(fillchar)
    return pad(s, 0, width - len(s), fill)


def rjust(s: str, width: int, fillchar: str | None = None) -> str:
    fill = fill_code_point(fillchar)
    return pad(s, width - len(s), 0, fill)


def zfill(s: str, width: int) -> str:
    n = len(s)
    if n >= width:
        return s
    zeros = width - n
    b = builder_new(width)
    i = 0
    while i < zeros:
        b[i] = 48
        i += 1
    put(b, zeros, s)
    # A sign goes before the zeros.
    if n > 0 and (code_point(s, 0) == 43 or code_point(s, 0) == 45):
        b[0] = code_point(s, 0)
        b[zeros] = 48
    return builder_done(b)


def expandtabs(s: str, tabsize: int | None = None) -> str:
    size = 8
    if tabsize is not None:
        size = tabsize
    total = expanded(s, size, None)
    b = builder_new(total)
    expanded(s, size, b)
    return builder_done(b)


def expanded(s: str, size: int, b: StrBuilder | None) -> int:
    """The length of `s` with each tab expanded to the next column that is a
    multiple of `size`, which also writes it to `b` when there is one. A
    line feed and a carriage return start the column again."""
    n = len(s)
    at = 0
    column = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        if c == 9:
            if size > 0:
                spaces = size - column % size
                if b is not None:
                    j = 0
                    while j < spaces:
                        b[at + j] = 32
                        j += 1
                at += spaces
                column += spaces
        else:
            if b is not None:
                b[at] = c
            at += 1
            column += 1
            if c == 10 or c == 13:
                column = 0
        i += 1
    return at


def fill_code_point(fillchar: str | None) -> int:
    """The code point that pads, which is a space without `fillchar`."""
    if fillchar is None:
        return 32
    if len(fillchar) != 1:
        raise TypeError("The fill character must be exactly one character long")
    return code_point(fillchar, 0)


def pad(s: str, left: int, right: int, fill: int) -> str:
    """`s` with `left` code points `fill` before it and `right` after it."""
    if left <= 0 and right <= 0:
        return s
    left = max(left, 0)
    right = max(right, 0)
    n = len(s)
    b = builder_new(left + n + right)
    i = 0
    while i < left:
        b[i] = fill
        i += 1
    put(b, left, s)
    i = left + n
    while i < left + n + right:
        b[i] = fill
        i += 1
    return builder_done(b)


def ord_of(c: str) -> int:
    """`ord(c)`."""
    n = len(c)
    if n != 1:
        raise TypeError(
            "ord() expected a character, but string of length " + str(n) + " found"
        )
    return code_point(c, 0)


def chr_of(i: int) -> str:
    """`chr(i)`."""
    if i < 0 or i > 0x10FFFF:
        raise ValueError("chr() arg not in range(0x110000)")
    if 0xD800 <= i <= 0xDFFF:
        stop(
            "chr("
            + str(i)
            + ") gives a surrogate, which a str of spython does not hold"
        )
    b = builder_new(1)
    b[0] = i
    return builder_done(b)


def is_in(sub: str, s: str) -> bool:
    return find(s, sub, None, None) >= 0


def slice_step(s: str, start: int | None, stop: int | None, step: int | None) -> str:
    """`s[start:stop:step]`, with the bounds that CPython adjusts to the step."""
    n = len(s)
    k = step_of(step)
    first = step_start(start, n, k)
    count = step_count(first, stop, n, k)
    b = builder_new(count)
    i = 0
    while i < count:
        b[i] = code_point(s, first + i * k)
        i += 1
    return builder_done(b)


def found_at(s: str, sub: str, i: int) -> bool:
    """Returns `True` if `sub` is in `s` from `i` on, `False` otherwise. The
    caller keeps `i + len(sub) <= len(s)`."""
    m = len(sub)
    j = 0
    while j < m:
        if code_point(s, i + j) != code_point(sub, j):
            return False
        j += 1
    return True


def casefold(s: str) -> str:
    n = len(s)
    size = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = 1
        while fold_at(c, k) >= 0:
            k += 1
        size += k
        i += 1
    b = builder_new(size)
    at = 0
    i = 0
    while i < n:
        c = code_point(s, i)
        k = 0
        m = fold_at(c, 0)
        while m >= 0:
            b[at] = m
            at += 1
            k += 1
            m = fold_at(c, k)
        i += 1
    return builder_done(b)


def capitalize(s: str) -> str:
    return recase(s, 0)


def title(s: str) -> str:
    return recase(s, 1)


def swapcase(s: str) -> str:
    return recase(s, 2)


def recase(s: str, method: int) -> str:
    """`s` after `capitalize` when `method` is 0, `title` when it is 1, and
    `swapcase` when it is 2."""
    n = len(s)
    size = 0
    i = 0
    while i < n:
        kind = case_kind(s, i, method)
        k = 1
        while case_mapped(s, i, kind, k) >= 0:
            k += 1
        size += k
        i += 1
    b = builder_new(size)
    at = 0
    i = 0
    while i < n:
        kind = case_kind(s, i, method)
        k = 0
        m = case_mapped(s, i, kind, 0)
        while m >= 0:
            b[at] = m
            at += 1
            k += 1
            m = case_mapped(s, i, kind, k)
        i += 1
    return builder_done(b)


def case_kind(s: str, i: int, method: int) -> int:
    """The case that `method` of `recase` gives to the code point at `i` of
    `s`: 0 for the same code point, 1 for upper, 2 for lower and 3 for
    title."""
    if method == 0:
        if i == 0:
            return 3
        return 2
    if method == 1:
        if i > 0 and is_cased(code_point(s, i - 1)):
            return 2
        return 3
    c = code_point(s, i)
    if is_uppercase(c):
        return 2
    if is_lowercase(c):
        return 1
    return 0


def case_mapped(s: str, i: int, kind: int, k: int) -> int:
    """The code point `k` of the case `kind` of the code point at `i` of
    `s`, or -1 past its end."""
    c = code_point(s, i)
    if kind == 0:
        if k == 0:
            return c
        return -1
    if kind == 1:
        return upper_at(c, k)
    if kind == 3:
        return title_at(c, k)
    if c == 931:
        if k == 0:
            return sigma(s, i)
        return -1
    return lower_at(c, k)


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


def put_hex_escape(b: StrBuilder, at: int, c: int, k: int) -> None:
    """Writes to `b` from `at` on the escape of `c` in `k` code points, which
    is `\\x` and 2 digits, `\\u` and 4, or `\\U` and 8."""
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


def hex_digit(d: int) -> int:
    if d < 10:
        return 48 + d
    return 87 + d
