"""The operators of `int` that need no instruction of their own."""

from __spython_prelude__.intrinsics import (
    code_point,
    decimal_value,
    mul_mod,
    natural_power,
    stop,
)
from __spython_prelude__.str import is_space, repr_of


def power(base: int, exp: int) -> int | float:
    """`base ** exp`, which is a `float` for a negative `exp`, as in Python."""
    if exp < 0:
        return float(base) ** float(exp)
    return natural_power(base, exp)


def power_mod(base: int, exp: int, mod: int) -> int:
    """`pow(base, exp, mod)`, as `long_pow` of CPython gives it. A negative
    `exp` takes the inverse of `base` modulo `mod`. Each value stays in the
    range of `% mod`, which is the range of the result."""
    if mod == 0:
        raise ValueError("pow() 3rd argument cannot be 0")
    if mod == 1 or mod == -1:
        return 0
    b = base % mod
    r = 1 % mod
    if exp < 0:
        b = inverse_mod(b, mod) % mod
        # `-exp` of the least `int` does not fit, so `r` takes one factor
        # first.
        r = b
        exp = -(exp + 1)
    while exp > 0:
        if exp % 2 == 1:
            r = mul_mod(r, b, mod)
        exp = exp // 2
        if exp > 0:
            b = mul_mod(b, b, mod)
    return r


def inverse_mod(a: int, mod: int) -> int:
    """A value whose product with `a` is 1 modulo `mod`, for an `a` in the
    range of `% mod`, by the algorithm of Euclid. Each remainder keeps the
    sign of `mod`, and `t1 * a` is `r1` modulo `mod`. The loop stops at a
    remainder of 1 or -1, before the last coefficient, which may not fit."""
    r0 = mod
    r1 = a
    t0 = 0
    t1 = 1
    while r1 != 1 and r1 != -1:
        if r1 == 0:
            raise ValueError("base is not invertible for the given modulus")
        q = r0 // r1
        r = r0 - q * r1
        r0 = r1
        r1 = r
        t = t0 - q * t1
        t0 = t1
        t1 = t
    return t1 * r1


def length(n: int) -> int:
    """`n`, the result of a method `__len__`, which `len` and the truth of an object take. A
    negative `n` raises `ValueError`."""
    if n < 0:
        raise ValueError("__len__() should return >= 0")
    return n


def int_of_str(s: str, base: int) -> int:
    """`int(s, base)`, as `PyLong_FromString` of CPython reads `s` after it
    takes each decimal digit to ASCII and each white space to a space."""
    if (base != 0 and base < 2) or base > 36:
        raise ValueError("int() base must be >= 2 and <= 36, or 0")
    # The error gives the base of the call, before a base of 0 becomes the
    # base of the prefix.
    given = base
    n = len(s)
    i = 0
    while is_ascii_space(ascii_at(s, i)):
        i += 1
    negative = False
    if ascii_at(s, i) == 43:
        i += 1
    elif ascii_at(s, i) == 45:
        negative = True
        i += 1
    # A 0 then a digit, the old octal, is an int with base 0 only if it is 0.
    zero_only = False
    if base == 0:
        base = 10
        if ascii_at(s, i) == 48:
            if is_prefix(ascii_at(s, i + 1), 16):
                base = 16
            elif is_prefix(ascii_at(s, i + 1), 8):
                base = 8
            elif is_prefix(ascii_at(s, i + 1), 2):
                base = 2
            else:
                zero_only = True
    if ascii_at(s, i) == 48 and is_prefix(ascii_at(s, i + 1), base):
        i += 2
        if ascii_at(s, i) == 95:
            i += 1
    if ascii_at(s, i) == 95:
        invalid(s, given)
    start = i
    digits = 0
    nonzero = False
    underscore = False
    while digit_value(ascii_at(s, i)) < base or ascii_at(s, i) == 95:
        if ascii_at(s, i) == 95:
            if underscore:
                invalid(s, given)
            underscore = True
        else:
            underscore = False
            digits += 1
            if digit_value(ascii_at(s, i)) != 0:
                nonzero = True
        i += 1
    if underscore:
        invalid(s, given)
    end = i
    if digits == 0:
        invalid(s, given)
    while is_ascii_space(ascii_at(s, i)):
        i += 1
    if i < n:
        invalid(s, given)
    if (
        digits > 4300
        and base != 2
        and base != 4
        and base != 8
        and base != 16
        and base != 32
    ):
        raise ValueError(
            "Exceeds the limit (4300 digits) for integer string conversion: value has "
            + str(digits)
            + " digits; use sys.set_int_max_str_digits() to increase the limit"
        )
    if zero_only and nonzero:
        invalid(s, given)
    # The value goes down from 0, since the least int has no opposite.
    value = 0
    i = start
    while i < end:
        d = digit_value(ascii_at(s, i))
        if d < base:
            if value < (-1073741824 + d + base - 1) // base:
                too_large()
            value = value * base - d
        i += 1
    if negative:
        return value
    if value == -1073741824:
        too_large()
    return -value


def ascii_at(s: str, i: int) -> int:
    """The code point at `i` of `s` as CPython reads it for an `int`: a
    decimal digit as its ASCII digit, a white space as a space, another code
    point past ASCII as `?`, and 0 past the end."""
    if i >= len(s):
        return 0
    c = code_point(s, i)
    if c < 127:
        return c
    if is_space(c):
        return 32
    d = decimal_value(c)
    if d >= 0:
        return 48 + d
    return 63


def is_ascii_space(c: int) -> bool:
    return c == 32 or 9 <= c <= 13


def is_prefix(c: int, base: int) -> bool:
    """Returns `True` if `c` is the letter after the 0 of the prefix of
    `base`, `False` otherwise."""
    if base == 16:
        return c == 120 or c == 88
    if base == 8:
        return c == 111 or c == 79
    if base == 2:
        return c == 98 or c == 66
    return False


def digit_value(c: int) -> int:
    """The value of the ASCII digit or letter `c`, and 37 for another code point."""
    if 48 <= c <= 57:
        return c - 48
    if 97 <= c <= 122:
        return c - 87
    if 65 <= c <= 90:
        return c - 55
    return 37


def invalid(s: str, base: int) -> None:
    text = repr_of(s)
    if len(text) > 200:
        text = text[:200]
    raise ValueError("invalid literal for int() with base " + str(base) + ": " + text)


def too_large() -> None:
    stop(
        "the result of int does not fit in an int, which goes from -1073741824 to 1073741823"
    )
