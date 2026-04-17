from enum import Enum, auto


class Color(Enum):
    '''The color of a traffic light.'''
    GREEN = auto()
    RED = auto()
    YELLOW = auto()


def next_color(current: Color) -> Color:
    '''
    Returns the next color of a traffic light that is currently *current*.

    Examples
    >>> next_color(Color.GREEN).name
    'YELLOW'
    >>> next_color(Color.YELLOW).name
    'RED'
    >>> next_color(Color.RED).name
    'GREEN'
    '''
    if current == Color.GREEN:
        result = Color.YELLOW
    elif current == Color.YELLOW:
        result = Color.RED
    elif current == Color.RED:
        result = Color.GREEN
    return result


assert next_color(Color.GREEN).name == 'YELLOW'
assert next_color(Color.YELLOW).name == 'RED'
assert next_color(Color.RED).name == 'GREEN'
