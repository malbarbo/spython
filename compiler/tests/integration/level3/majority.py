from enum import Enum, auto


class Majority(Enum):
    POSITIVES = auto()
    NEGATIVES = auto()
    NONE = auto()


def majority(lst: list[int]) -> Majority:
    '''
    Returns whether the majority of elements in *lst* are positive, negative, or neither.

    Examples
    >>> majority([]).name
    'NONE'
    >>> majority([0, 4, 0, 2, 0]).name
    'POSITIVES'
    >>> majority([0, -4, 0, -2, 0]).name
    'NEGATIVES'
    >>> majority([0, -4, 0, 2, 0]).name
    'NONE'
    '''
    positives = 0
    negatives = 0
    for n in lst:
        if n > 0:
            positives = positives + 1
        elif n < 0:
            negatives = negatives + 1

    if positives > negatives:
        m = Majority.POSITIVES
    elif negatives > positives:
        m = Majority.NEGATIVES
    else:
        m = Majority.NONE
    return m


assert majority([]).name == 'NONE'
assert majority([0, 4, 0, 2, 0]).name == 'POSITIVES'
assert majority([0, -4, 0, -2, 0]).name == 'NEGATIVES'
assert majority([0, -4, 0, 2, 0]).name == 'NONE'
assert majority([1, 2, 3]).name == 'POSITIVES'
assert majority([-1, -2, -3]).name == 'NEGATIVES'
