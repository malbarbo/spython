"""The builtin functions that the compiler takes from Python, each under its
own name. A call of one of them binds its arguments as a call of a function
of the program does."""

from __spython_prelude__.intrinsics import builder_done, builder_new
from __spython_prelude__.str import hex_digit


def bin(number: int) -> str:
    return in_base(number, 1, 98)


def oct(number: int) -> str:
    return in_base(number, 3, 111)


def hex(number: int) -> str:
    return in_base(number, 4, 120)


def in_base(n: int, bits: int, letter: int) -> str:
    """`n` in the base `2 ** bits`, after a 0 and the code point `letter`,
    with a `-` before a negative `n`."""
    base = 1 << bits
    # The digits come from a value that is not positive, since the least
    # `int` has no opposite.
    v = n
    if n > 0:
        v = -n
    # `v - v % -base` is `v` without its last digit, a multiple of `base`.
    digits = 1
    rest = (v - v % -base) // base
    while rest != 0:
        digits += 1
        rest = (rest - rest % -base) // base
    sign = 0
    if n < 0:
        sign = 1
    b = builder_new(sign + 2 + digits)
    if n < 0:
        b[0] = 45
    b[sign] = 48
    b[sign + 1] = letter
    at = sign + 1 + digits
    while at > sign + 1:
        b[at] = hex_digit(-(v % -base))
        v = (v - v % -base) // base
        at -= 1
    return builder_done(b)
