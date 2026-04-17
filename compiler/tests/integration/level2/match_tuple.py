def classify_point(point: list[int]) -> str:
    '''
    Classifies the 2D point *point* (given as [x, y]).

    Examples
    >>> classify_point([0, 0])
    'origin'
    >>> classify_point([3, 0])
    'x-axis'
    >>> classify_point([0, 5])
    'y-axis'
    >>> classify_point([4, 4])
    'diagonal'
    >>> classify_point([2, 7])
    'general'
    '''
    match point:
        case [0, 0]:
            kind = 'origin'
        case [_, 0]:
            kind = 'x-axis'
        case [0, _]:
            kind = 'y-axis'
        case [x, y] if x == y:
            kind = 'diagonal'
        case [_, _]:
            kind = 'general'
        case _:
            kind = 'invalid'
    return kind


def swap(pair: list[int]) -> list[int]:
    '''
    Swaps the two elements of *pair*.

    Examples
    >>> swap([1, 2])
    [2, 1]
    >>> swap([0, 0])
    [0, 0]
    '''
    match pair:
        case [a, b]:
            result = [b, a]
        case _:
            result = pair
    return result


assert classify_point([0, 0]) == 'origin'
assert classify_point([3, 0]) == 'x-axis'
assert classify_point([0, 5]) == 'y-axis'
assert classify_point([4, 4]) == 'diagonal'
assert classify_point([2, 7]) == 'general'
assert swap([1, 2]) == [2, 1]
assert swap([0, 0]) == [0, 0]
