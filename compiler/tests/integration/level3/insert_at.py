def insert_at(lst: list[int], i: int, n: int) -> list[int]:
    '''
    Returns a new list with *n* inserted at position *i* of *lst*.
    Requires 0 <= i <= len(lst).

    Examples
    >>> insert_at([], 0, 10)
    [10]
    >>> insert_at([5], 0, 10)
    [10, 5]
    >>> insert_at([5], 1, 10)
    [5, 10]
    >>> insert_at([5, 7], 1, 10)
    [5, 10, 7]
    >>> insert_at([5, 7], 2, 10)
    [5, 7, 10]
    '''
    assert 0 <= i <= len(lst)
    r = []
    j = 0
    while j < i:
        r.append(lst[j])
        j = j + 1
    r.append(n)
    while j < len(lst):
        r.append(lst[j])
        j = j + 1
    return r


assert insert_at([], 0, 10) == [10]
assert insert_at([5], 0, 10) == [10, 5]
assert insert_at([5], 1, 10) == [5, 10]
assert insert_at([5, 7], 0, 10) == [10, 5, 7]
assert insert_at([5, 7], 1, 10) == [5, 10, 7]
assert insert_at([5, 7], 2, 10) == [5, 7, 10]
