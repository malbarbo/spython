from dataclasses import dataclass
from enum import Enum, auto


@dataclass
class Position:
    x: int
    y: int
    z: int


class Move(Enum):
    NORTH = auto()
    SOUTH = auto()
    EAST = auto()
    WEST = auto()
    UP = auto()
    DOWN = auto()


def new_position(p: Position, moves: list[Move]) -> Position:
    '''
    Returns the new position after applying all *moves* starting from *p*.
    NORTH/SOUTH affect x (+/-), EAST/WEST affect y (+/-), UP/DOWN affect z (+/-).

    Examples
    >>> D = Move
    >>> new_position(Position(6, 1, 3), [])
    Position(x=6, y=1, z=3)
    >>> new_position(Position(6, 1, 3), [D.UP, D.NORTH, D.EAST, D.UP, D.UP, D.NORTH])
    Position(x=8, y=2, z=6)
    >>> new_position(Position(6, 1, 3), [D.DOWN, D.SOUTH, D.WEST, D.DOWN, D.DOWN, D.SOUTH])
    Position(x=4, y=0, z=0)
    '''
    x = p.x
    y = p.y
    z = p.z
    for d in moves:
        if d == Move.NORTH:
            x = x + 1
        elif d == Move.SOUTH:
            x = x - 1
        elif d == Move.EAST:
            y = y + 1
        elif d == Move.WEST:
            y = y - 1
        elif d == Move.UP:
            z = z + 1
        elif d == Move.DOWN:
            z = z - 1
    return Position(x, y, z)


assert new_position(Position(6, 1, 3), []) == Position(6, 1, 3)
assert new_position(Position(6, 1, 3), [Move.UP, Move.NORTH, Move.EAST, Move.UP, Move.UP, Move.NORTH]) == Position(8, 2, 6)
assert new_position(Position(6, 1, 3), [Move.DOWN, Move.SOUTH, Move.WEST, Move.DOWN, Move.DOWN, Move.SOUTH]) == Position(4, 0, 0)
