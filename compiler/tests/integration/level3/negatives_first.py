def negatives_first(lst: list[int]) -> list[int]:
    '''
    Returns a new list with negatives first, then zeros, then positives.

    Examples
    >>> negatives_first([])
    []
    >>> negatives_first([3, 0, -1, 0, 4])
    [-1, 0, 0, 3, 4]
    '''
    negatives = []
    zeros = []
    positives = []
    for n in lst:
        if n < 0:
            negatives.append(n)
        elif n == 0:
            zeros.append(n)
        else:
            positives.append(n)
    return negatives + zeros + positives


assert negatives_first([]) == []
assert negatives_first([3, 0, -1, 0, 4]) == [-1, 0, 0, 3, 4]
assert negatives_first([-1, -2, -3]) == [-1, -2, -3]
assert negatives_first([1, 2, 3]) == [1, 2, 3]
assert negatives_first([0]) == [0]
