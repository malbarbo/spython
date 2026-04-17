from dataclasses import dataclass
from enum import Enum, auto


class Direction(Enum):
    NORTH = auto()
    EAST = auto()
    SOUTH = auto()
    WEST = auto()


@dataclass
class Character:
    row: int
    col: int
    dir: Direction


def max_steps(p: Character) -> int:
    '''
    Returns the maximum number of steps the character *p* can advance
    on a 10x10 board (rows and columns numbered 1 to 10).

    Examples
    >>> max_steps(Character(row=4, col=2, dir=Direction.NORTH))
    6
    >>> max_steps(Character(row=10, col=2, dir=Direction.NORTH))
    0
    >>> max_steps(Character(row=4, col=2, dir=Direction.SOUTH))
    3
    >>> max_steps(Character(row=1, col=2, dir=Direction.SOUTH))
    0
    >>> max_steps(Character(row=4, col=2, dir=Direction.EAST))
    8
    >>> max_steps(Character(row=4, col=10, dir=Direction.EAST))
    0
    >>> max_steps(Character(row=4, col=2, dir=Direction.WEST))
    1
    >>> max_steps(Character(row=4, col=1, dir=Direction.WEST))
    0
    '''
    if p.dir == Direction.NORTH:
        steps = 10 - p.row
    elif p.dir == Direction.SOUTH:
        steps = p.row - 1
    elif p.dir == Direction.EAST:
        steps = 10 - p.col
    elif p.dir == Direction.WEST:
        steps = p.col - 1
    return steps


assert max_steps(Character(row=4, col=2, dir=Direction.NORTH)) == 6
assert max_steps(Character(row=10, col=2, dir=Direction.NORTH)) == 0
assert max_steps(Character(row=4, col=2, dir=Direction.SOUTH)) == 3
assert max_steps(Character(row=1, col=2, dir=Direction.SOUTH)) == 0
assert max_steps(Character(row=4, col=2, dir=Direction.EAST)) == 8
assert max_steps(Character(row=4, col=10, dir=Direction.EAST)) == 0
assert max_steps(Character(row=4, col=2, dir=Direction.WEST)) == 1
assert max_steps(Character(row=4, col=1, dir=Direction.WEST)) == 0
