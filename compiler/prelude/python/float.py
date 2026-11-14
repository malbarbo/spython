"""The functions of `float` that need no instruction of their own."""

from __spython_prelude__.int import ascii_at, is_ascii_space
from __spython_prelude__.intrinsics import (
    builder_done,
    builder_new,
    code_point,
    parse_float,
)
from __spython_prelude__.str import hex_digit, repr_of


def is_integer(x: float) -> bool:
    """`float.is_integer`. An infinity and a NaN leave a NaN as the rest."""
    return x % 1.0 == 0.0


def real(x: float) -> float:
    return x


def imag(x: float) -> float:
    return 0.0


def conjugate(x: float) -> float:
    return x


def hex(x: float) -> str:
    """`float.hex`, which writes `x` as `m * 2 ** e`, with the 52 bits after
    the point of `m` in hexadecimal. `m` is from 1 up to 2, or below 1 for
    a subnormal `x`, whose `e` is -1022."""
    if x != x:
        return "nan"
    sign = ""
    # Only its `str` tells -0.0 from 0.0.
    if x < 0.0 or (x == 0.0 and str(x) == "-0.0"):
        sign = "-"
        x = -x
    if x - x != 0.0:
        return sign + "inf"
    if x == 0.0:
        return sign + "0x0.0p+0"
    # A product by a power of 2 is exact, and the steps from 512 down
    # take `e` to any exponent of a `float`.
    e = 0
    step = 512
    while step > 0:
        power = 2.0**step
        while x >= 2.0 and x >= power:
            x /= power
            e += step
        while x < 1.0 and e - step >= -1022 and x * power < 2.0:
            x *= power
            e -= step
        step //= 2
    lead = int(x)
    rest = x - lead
    digits = ""
    for _ in range(13):
        rest *= 16.0
        d = int(rest)
        rest -= d
        digits += chr(hex_digit(d))
    exponent = "+" + str(e)
    if e < 0:
        exponent = str(e)
    return sign + "0x" + str(lead) + "." + digits + "p" + exponent


def fromhex(string: str) -> float:
    """`float.fromhex`, which rounds the bits past the 53 of a `float` to
    the nearest, and to the even one at the middle. Only ASCII counts as a
    digit or a white space."""
    s = string
    end = len(s)
    i = 0
    while is_ascii_space(char_at(s, i)):
        i += 1
    while end > i and is_ascii_space(char_at(s, end - 1)):
        end -= 1
    negative = char_at(s, i) == 45
    if negative or char_at(s, i) == 43:
        i += 1
    inf = 1e308 * 10.0
    if word_at(s, i, end, "inf") or word_at(s, i, end, "infinity"):
        if negative:
            return -inf
        return inf
    if word_at(s, i, end, "nan"):
        return inf - inf
    if char_at(s, i) == 48 and (char_at(s, i + 1) == 120 or char_at(s, i + 1) == 88):
        i += 2
    # The value is the integer of `digits` times `2 ** exp`.
    digits: list[int] = []
    exp = 0
    seen = False
    point = False
    while i < end:
        c = char_at(s, i)
        d = hex_value(c)
        if d >= 0:
            seen = True
            if d > 0 or len(digits) > 0:
                digits.append(d)
            if point:
                exp -= 4
        elif c == 46 and not point:
            point = True
        else:
            break
        i += 1
    if not seen:
        raise ValueError("invalid hexadecimal floating-point string")
    if i < end and (char_at(s, i) == 112 or char_at(s, i) == 80):
        i += 1
        exponent_negative = char_at(s, i) == 45
        if exponent_negative or char_at(s, i) == 43:
            i += 1
        start = i
        # The digits move the point by at most `4 * len(s)` bits, so an
        # exponent past `limit` gives 0 or an overflow all the same.
        limit = 4 * len(s) + 1100
        e = 0
        while i < end and 48 <= char_at(s, i) <= 57:
            d = char_at(s, i) - 48
            if e > (limit - d) // 10:
                e = limit + 1
            else:
                e = 10 * e + d
            i += 1
        if i == start:
            raise ValueError("invalid hexadecimal floating-point string")
        if exponent_negative:
            e = -e
        exp += e
    if i != end:
        raise ValueError("invalid hexadecimal floating-point string")
    x = 0.0
    if len(digits) > 0:
        x = from_bits(digits, exp)
    if negative:
        return -x
    return x


def char_at(s: str, i: int) -> int:
    """The code point at `i` of `s`, or 0 past its end."""
    if i >= len(s):
        return 0
    return code_point(s, i)


def from_bits(digits: list[int], exp: int) -> float:
    """The `float` nearest the integer of the hexadecimal `digits` times
    `2 ** exp`. The first digit is not 0."""
    first = 0
    d = digits[0]
    while d > 0:
        d //= 2
        first += 1
    size = first + 4 * (len(digits) - 1)
    top = size - 1 + exp
    if top > 1023:
        raise OverflowError("hexadecimal value too large to represent as a float")
    # A subnormal keeps fewer bits, and a value below half the least
    # subnormal keeps none.
    keep = 53
    if top < -1022:
        keep = top + 1075
    if keep < 0:
        return 0.0
    mantissa = 0.0
    for k in range(keep):
        mantissa *= 2.0
        if k < size:
            mantissa += bit(digits, first, k)
    if keep < size and bit(digits, first, keep) == 1:
        if mantissa % 2.0 == 1.0 or any_bit_after(digits, first, keep):
            mantissa += 1.0
    x = mantissa * 2.0 ** (top - keep + 1)
    if x - x != 0.0:
        raise OverflowError("hexadecimal value too large to represent as a float")
    return x


def bit(digits: list[int], first: int, k: int) -> int:
    """The bit `k` from the left of the integer of `digits`, whose first
    digit has `first` bits."""
    if k < first:
        return (digits[0] >> (first - 1 - k)) & 1
    j = k - first
    return (digits[1 + j // 4] >> (3 - j % 4)) & 1


def any_bit_after(digits: list[int], first: int, k: int) -> bool:
    """Returns `True` if a bit after the bit `k` from the left of the integer
    of `digits` is 1, `False` otherwise."""
    for j in range(k + 1, first + 4 * (len(digits) - 1)):
        if bit(digits, first, j) == 1:
            return True
    return False


def hex_value(c: int) -> int:
    """The value of the code point `c` as a hexadecimal digit, or -1."""
    if 48 <= c <= 57:
        return c - 48
    if 97 <= c <= 102:
        return c - 87
    if 65 <= c <= 70:
        return c - 55
    return -1


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
