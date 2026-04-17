def remove_zeros(lst: list[int]) -> list[int]:
    '''
    Returns a new list with all zero values removed from *lst*.

    Examples
    >>> remove_zeros([])
    []
    >>> remove_zeros([4, 1, 0, 3, 0])
    [4, 1, 3]
    '''
    result = []
    for n in lst:
        if n != 0:
            result.append(n)
    return result


assert remove_zeros([]) == []
assert remove_zeros([4, 1, 0, 3, 0]) == [4, 1, 3]
assert remove_zeros([0, 0, 0]) == []
assert remove_zeros([1, 2, 3]) == [1, 2, 3]
