from enum import Enum, auto


class Direction(Enum):
    NORTH = auto()
    EAST = auto()
    SOUTH = auto()
    WEST = auto()


def opposite(d: Direction) -> Direction:
    '''
    Returns the direction opposite to *d*.

    Examples
    >>> opposite(Direction.NORTH).name
    'SOUTH'
    >>> opposite(Direction.SOUTH).name
    'NORTH'
    >>> opposite(Direction.EAST).name
    'WEST'
    >>> opposite(Direction.WEST).name
    'EAST'
    '''
    if d == Direction.NORTH:
        do = Direction.SOUTH
    elif d == Direction.SOUTH:
        do = Direction.NORTH
    elif d == Direction.EAST:
        do = Direction.WEST
    elif d == Direction.WEST:
        do = Direction.EAST
    return do


def rotate_clockwise_90(d: Direction) -> Direction:
    '''
    Returns the direction 90 degrees clockwise from *d*.

    Examples
    >>> rotate_clockwise_90(Direction.NORTH).name
    'EAST'
    >>> rotate_clockwise_90(Direction.EAST).name
    'SOUTH'
    >>> rotate_clockwise_90(Direction.SOUTH).name
    'WEST'
    >>> rotate_clockwise_90(Direction.WEST).name
    'NORTH'
    '''
    if d == Direction.NORTH:
        dh = Direction.EAST
    elif d == Direction.EAST:
        dh = Direction.SOUTH
    elif d == Direction.SOUTH:
        dh = Direction.WEST
    elif d == Direction.WEST:
        dh = Direction.NORTH
    return dh


def rotate_counter_clockwise_90(d: Direction) -> Direction:
    '''
    Returns the direction 90 degrees counter-clockwise from *d*.

    Examples
    >>> rotate_counter_clockwise_90(Direction.NORTH).name
    'WEST'
    >>> rotate_counter_clockwise_90(Direction.EAST).name
    'NORTH'
    >>> rotate_counter_clockwise_90(Direction.SOUTH).name
    'EAST'
    >>> rotate_counter_clockwise_90(Direction.WEST).name
    'SOUTH'
    '''
    return rotate_clockwise_90(rotate_clockwise_90(rotate_clockwise_90(d)))


assert opposite(Direction.NORTH).name == 'SOUTH'
assert opposite(Direction.SOUTH).name == 'NORTH'
assert opposite(Direction.EAST).name == 'WEST'
assert opposite(Direction.WEST).name == 'EAST'
assert rotate_clockwise_90(Direction.NORTH).name == 'EAST'
assert rotate_clockwise_90(Direction.EAST).name == 'SOUTH'
assert rotate_clockwise_90(Direction.SOUTH).name == 'WEST'
assert rotate_clockwise_90(Direction.WEST).name == 'NORTH'
assert rotate_counter_clockwise_90(Direction.NORTH).name == 'WEST'
assert rotate_counter_clockwise_90(Direction.EAST).name == 'NORTH'
assert rotate_counter_clockwise_90(Direction.SOUTH).name == 'EAST'
assert rotate_counter_clockwise_90(Direction.WEST).name == 'SOUTH'
