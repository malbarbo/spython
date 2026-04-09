def maximo(lst: list[int]) -> int:
    '''
    Encontra o valor máximo de *lst*.
    Requer que len(lst) > 0.

    Exemplos
    >>> maximo([3])
    3
    >>> maximo([3, 2])
    3
    >>> maximo([3, 2, 4])
    4
    >>> maximo([3, 2, 4, 1])
    4
    '''
    assert len(lst) > 0
    if len(lst) == 1:
        m = lst[0]
    else:
        m = max(lst[0], maximo(lst[1:]))
    return m

# Generated from doctests.
assert maximo([3]) == 3
assert maximo([3, 2]) == 3
assert maximo([3, 2, 4]) == 4
assert maximo([3, 2, 4, 1]) == 4
