"""The functions of `float` that need no instruction of their own."""

from __spython_prelude__.int import ascii_at, is_ascii_space
from __spython_prelude__.intrinsics import (
    builder_done,
    builder_new,
    code_point,
    parse_float,
)
from __spython_prelude__.str import repr_of


def float_of_str(s: str) -> float:
    """`float(s)`, which takes each decimal digit to ASCII and each white
    space to a space, as `int(s)` does. An underscore goes between two
    digits alone."""
    first = 0
    while is_ascii_space(ascii_at(s, first)):
        first += 1
    end = len(s)
    while end > first and is_ascii_space(ascii_at(s, end - 1)):
        end -= 1
    if float_end(s, first, end) != end:
        raise ValueError("could not convert string to float: " + repr_of(s))
    size = 0
    i = first
    while i < end:
        if ascii_at(s, i) != 95:
            size += 1
        i += 1
    b = builder_new(size)
    at = 0
    i = first
    while i < end:
        c = ascii_at(s, i)
        if c != 95:
            b[at] = c
            at += 1
        i += 1
    return parse_float(builder_done(b))


def float_end(s: str, i: int, end: int) -> int:
    """The index after the float of Python at `i` of `s`, which stops at
    `end`, or -1 when there is none."""
    if ascii_at(s, i) == 43 or ascii_at(s, i) == 45:
        i += 1
    if (
        word_at(s, i, end, "infinity")
        or word_at(s, i, end, "inf")
        or word_at(s, i, end, "nan")
    ):
        return end
    whole = digits_end(s, i, end)
    digits = whole - i
    i = whole
    if i < end and ascii_at(s, i) == 46:
        fraction = digits_end(s, i + 1, end)
        digits += fraction - i - 1
        i = fraction
    if digits == 0:
        return -1
    if i < end and (ascii_at(s, i) == 101 or ascii_at(s, i) == 69):
        j = i + 1
        if ascii_at(s, j) == 43 or ascii_at(s, j) == 45:
            j += 1
        exponent = digits_end(s, j, end)
        if exponent > j:
            i = exponent
    return i


def digits_end(s: str, i: int, end: int) -> int:
    """The index after the digits at `i` of `s`, with an underscore between
    two of them."""
    while i < end and is_digit(ascii_at(s, i)):
        i += 1
        if i + 1 < end and ascii_at(s, i) == 95 and is_digit(ascii_at(s, i + 1)):
            i += 1
    return i


def is_digit(c: int) -> bool:
    return 48 <= c <= 57


def word_at(s: str, i: int, end: int, word: str) -> bool:
    """Returns `True` if `s` from `i` up to `end` is `word`, a word of small
    ASCII letters, in any case, `False` otherwise."""
    n = len(word)
    if end - i != n:
        return False
    j = 0
    while j < n:
        c = ascii_at(s, i + j)
        if c != code_point(word, j) and c != code_point(word, j) - 32:
            return False
        j += 1
    return True
