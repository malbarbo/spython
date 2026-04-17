import math


def tile_count(length: float, height: float) -> int:
    '''
    Calculates the number of 0.2m x 0.2m tiles needed to cover a wall
    of size *length* x *height* (in meters), assuming no tile is wasted
    and cut pieces are discarded.

    Examples
    >>> # no cuts: 10 (2.0 / 0.2) x 12 (2.4 / 0.2)
    >>> tile_count(2.0, 2.4)
    120
    >>> # with cuts: 8 (ceil(1.5 / 0.2)) x 12 (ceil(2.3 / 0.2))
    >>> tile_count(1.5, 2.3)
    96

    Edge cases
    >>> tile_count(0.2, 0.2)
    1
    >>> tile_count(0.3, 0.2)
    2
    >>> tile_count(0.3, 0.3)
    4
    >>> tile_count(0.4, 0.4)
    4
    '''
    return math.ceil(length / 0.2) * math.ceil(height / 0.2)


assert tile_count(2.0, 2.4) == 120
assert tile_count(1.5, 2.3) == 96
assert tile_count(0.2, 0.2) == 1
assert tile_count(0.3, 0.2) == 2
assert tile_count(0.3, 0.3) == 4
assert tile_count(0.4, 0.4) == 4
