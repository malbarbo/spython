def remove_at(lst: list[int], i: int) -> list[int]:
    '''
    Returns a new list with the element at position *i* removed.
    Requires 0 <= i < len(lst).

    Examples
    >>> remove_at([3], 0)
    []
    >>> remove_at([3, 5, 1], 0)
    [5, 1]
    >>> remove_at([3, 5, 1], 1)
    [3, 1]
    >>> remove_at([3, 5, 1], 2)
    [3, 5]
    '''
    assert 0 <= i < len(lst)
    result = []
    j = 0
    while j < i:
        result.append(lst[j])
        j = j + 1
    j = i + 1
    while j < len(lst):
        result.append(lst[j])
        j = j + 1
    return result


assert remove_at([3], 0) == []
assert remove_at([3, 5, 1], 0) == [5, 1]
assert remove_at([3, 5, 1], 1) == [3, 1]
assert remove_at([3, 5, 1], 2) == [3, 5]
