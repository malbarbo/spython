def list_sum(lst: list[int]) -> int:
    '''
    Sums the elements of *lst*.

    Examples
    >>> list_sum([])
    0
    >>> list_sum([3])
    3
    >>> list_sum([3, 7])
    10
    >>> list_sum([3, 7, 2])
    12
    '''
    total = 0
    for n in lst:
        total = total + n
    return total


assert list_sum([]) == 0
assert list_sum([3]) == 3
assert list_sum([3, 7]) == 10
assert list_sum([3, 7, 2]) == 12
