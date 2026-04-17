from enum import Enum, auto


class Shape(Enum):
    CIRCLE = auto()
    SQUARE = auto()
    TRIANGLE = auto()


def sides(s: Shape) -> int:
    '''
    Returns the number of sides of shape *s*.
    Circles have 0 sides.

    Examples
    >>> sides(Shape.CIRCLE)
    0
    >>> sides(Shape.SQUARE)
    4
    >>> sides(Shape.TRIANGLE)
    3
    '''
    match s:
        case Shape.CIRCLE:
            n = 0
        case Shape.SQUARE:
            n = 4
        case Shape.TRIANGLE:
            n = 3
    return n


def describe_number(n: int) -> str:
    '''
    Returns a description of *n*.

    Examples
    >>> describe_number(0)
    'zero'
    >>> describe_number(1)
    'one'
    >>> describe_number(42)
    'other: 42'
    '''
    match n:
        case 0:
            desc = 'zero'
        case 1:
            desc = 'one'
        case other:
            desc = 'other: ' + str(other)
    return desc


assert sides(Shape.CIRCLE) == 0
assert sides(Shape.SQUARE) == 4
assert sides(Shape.TRIANGLE) == 3
assert describe_number(0) == 'zero'
assert describe_number(1) == 'one'
assert describe_number(42) == 'other: 42'
