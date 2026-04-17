from dataclasses import dataclass


@dataclass
class SixNumbers:
    '''A collection of 6 distinct numbers between 1 and 60.'''
    a: int
    b: int
    c: int
    d: int
    e: int
    f: int


def hit_count(bet: SixNumbers, drawn: SixNumbers) -> int:
    '''
    Counts how many numbers in *bet* appear in *drawn*.

    Examples
    >>> hit_count(SixNumbers(1, 2, 3, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57))
    0
    >>> hit_count(SixNumbers(8, 2, 3, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57))
    1
    >>> hit_count(SixNumbers(8, 12, 20, 41, 52, 57), SixNumbers(8, 12, 20, 41, 52, 57))
    6
    '''
    hits = 0

    if is_drawn(bet.a, drawn):
        hits = hits + 1
    if is_drawn(bet.b, drawn):
        hits = hits + 1
    if is_drawn(bet.c, drawn):
        hits = hits + 1
    if is_drawn(bet.d, drawn):
        hits = hits + 1
    if is_drawn(bet.e, drawn):
        hits = hits + 1
    if is_drawn(bet.f, drawn):
        hits = hits + 1

    return hits


def is_drawn(n: int, drawn: SixNumbers) -> bool:
    '''
    Returns True if *n* is one of the numbers in *drawn*.

    Examples
    >>> d = SixNumbers(1, 7, 10, 40, 41, 60)
    >>> is_drawn(1, d)
    True
    >>> is_drawn(7, d)
    True
    >>> is_drawn(2, d)
    False
    '''
    in_drawn = False

    if n == drawn.a:
        in_drawn = True
    if n == drawn.b:
        in_drawn = True
    if n == drawn.c:
        in_drawn = True
    if n == drawn.d:
        in_drawn = True
    if n == drawn.e:
        in_drawn = True
    if n == drawn.f:
        in_drawn = True

    return in_drawn


assert hit_count(SixNumbers(1, 2, 3, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 0
assert hit_count(SixNumbers(8, 2, 3, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 1
assert hit_count(SixNumbers(8, 12, 3, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 2
assert hit_count(SixNumbers(8, 12, 20, 4, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 3
assert hit_count(SixNumbers(8, 12, 20, 41, 5, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 4
assert hit_count(SixNumbers(8, 12, 20, 41, 52, 6), SixNumbers(8, 12, 20, 41, 52, 57)) == 5
assert hit_count(SixNumbers(8, 12, 20, 41, 52, 57), SixNumbers(8, 12, 20, 41, 52, 57)) == 6
_d = SixNumbers(1, 7, 10, 40, 41, 60)
assert is_drawn(1, _d) == True
assert is_drawn(7, _d) == True
assert is_drawn(10, _d) == True
assert is_drawn(40, _d) == True
assert is_drawn(41, _d) == True
assert is_drawn(60, _d) == True
assert is_drawn(2, _d) == False
