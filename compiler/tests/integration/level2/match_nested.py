from dataclasses import dataclass


@dataclass
class Range:
    low: int
    high: int


def classify_segment(seg: list[list[int]]) -> str:
    '''
    Classifies a segment given as [[x1, y1], [x2, y2]].

    Examples
    >>> classify_segment([[0, 0], [0, 0]])
    'point'
    >>> classify_segment([[1, 3], [4, 3]])
    'horizontal'
    >>> classify_segment([[2, 1], [2, 5]])
    'vertical'
    >>> classify_segment([[0, 0], [3, 4]])
    'other'
    '''
    match seg:
        case [[x1, y1], [x2, y2]] if x1 == x2 and y1 == y2:
            kind = 'point'
        case [[_, y1], [_, y2]] if y1 == y2:
            kind = 'horizontal'
        case [[x1, _], [x2, _]] if x1 == x2:
            kind = 'vertical'
        case _:
            kind = 'other'
    return kind


def range_contains(r: Range, intervals: list[list[int]]) -> bool:
    '''
    Returns True if *r* is fully contained within any interval in *intervals*.
    Each interval is [low, high].

    Examples
    >>> range_contains(Range(3, 7), [[0, 10], [20, 30]])
    True
    >>> range_contains(Range(3, 7), [[5, 10], [20, 30]])
    False
    >>> range_contains(Range(3, 7), [])
    False
    '''
    found = False
    for interval in intervals:
        match interval:
            case [lo, hi] if lo <= r.low and r.high <= hi:
                found = True
    return found


assert classify_segment([[0, 0], [0, 0]]) == 'point'
assert classify_segment([[1, 3], [4, 3]]) == 'horizontal'
assert classify_segment([[2, 1], [2, 5]]) == 'vertical'
assert classify_segment([[0, 0], [3, 4]]) == 'other'
assert range_contains(Range(3, 7), [[0, 10], [20, 30]]) == True
assert range_contains(Range(3, 7), [[5, 10], [20, 30]]) == False
assert range_contains(Range(3, 7), []) == False
