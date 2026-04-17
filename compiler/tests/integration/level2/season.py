from enum import Enum, auto


class Season(Enum):
    SPRING = auto()
    SUMMER = auto()
    AUTUMN = auto()
    WINTER = auto()


def is_warm(s: Season) -> bool:
    '''
    Returns True if *s* is a warm season (spring or summer).

    Examples
    >>> is_warm(Season.SPRING)
    True
    >>> is_warm(Season.SUMMER)
    True
    >>> is_warm(Season.AUTUMN)
    False
    >>> is_warm(Season.WINTER)
    False
    '''
    match s:
        case Season.SPRING:
            result = True
        case Season.SUMMER:
            result = True
        case _:
            result = False
    return result


assert is_warm(Season.SPRING) == True
assert is_warm(Season.SUMMER) == True
assert is_warm(Season.AUTUMN) == False
assert is_warm(Season.WINTER) == False
