from dataclasses import dataclass
from enum import Enum, auto


class Color(Enum):
    RED = auto()
    GREEN = auto()
    BLUE = auto()


@dataclass
class Pixel:
    x: int
    y: int
    color: Color


def on_diagonal(p: Pixel) -> bool:
    '''
    Returns True if *p* is on the main diagonal (x == y), regardless of color.

    Examples
    >>> on_diagonal(Pixel(3, 3, Color.RED))
    True
    >>> on_diagonal(Pixel(1, 2, Color.GREEN))
    False
    '''
    match p:
        case Pixel(x=x, y=y, color=_) if x == y:
            result = True
        case _:
            result = False
    return result


def describe_pixel(p: Pixel) -> str:
    '''
    Returns a description combining position and color name.

    Examples
    >>> describe_pixel(Pixel(0, 0, Color.RED))
    'origin red'
    >>> describe_pixel(Pixel(1, 0, Color.GREEN))
    '(1, 0) green'
    >>> describe_pixel(Pixel(2, 3, Color.BLUE))
    '(2, 3) blue'
    '''
    match p:
        case Pixel(x=0, y=0, color=c):
            desc = 'origin ' + c.name.lower()
        case Pixel(x=x, y=y, color=c):
            desc = '(' + str(x) + ', ' + str(y) + ') ' + c.name.lower()
    return desc


assert on_diagonal(Pixel(3, 3, Color.RED)) == True
assert on_diagonal(Pixel(1, 2, Color.GREEN)) == False
assert on_diagonal(Pixel(0, 0, Color.BLUE)) == True
assert describe_pixel(Pixel(0, 0, Color.RED)) == 'origin red'
assert describe_pixel(Pixel(1, 0, Color.GREEN)) == '(1, 0) green'
assert describe_pixel(Pixel(2, 3, Color.BLUE)) == '(2, 3) blue'
