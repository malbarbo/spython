def maximo(lst: list[int]) -> int:
    '''
    Encontra o valor máximo de *lst*.
    Requer que lst seja não vazia.

    Exemplos
    >>> maximo([2])
    2
    >>> maximo([2, 4])
    4
    >>> maximo([2, 4, 3])
    4
    >>> maximo([2, 4, 3, 7])
    7
    '''
    assert len(lst) != 0
    maximo = lst[0]
    for n in lst:
        if n > maximo:
            maximo = n
    return maximo

# Generated from doctests.
assert maximo([2]) == 2
assert maximo([2, 4]) == 4
assert maximo([2, 4, 3]) == 4
assert maximo([2, 4, 3, 7]) == 7
