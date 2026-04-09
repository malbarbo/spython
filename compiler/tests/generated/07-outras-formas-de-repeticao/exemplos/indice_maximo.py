def indice_maximo(lst: list[int]) -> int:
    '''
    Encontra o índice da primeira ocorrência do valor máximo de *lst*.
    Requer que *lst* seja não vazia.

    Exemplos
    >>> indice_maximo([5])
    0
    >>> indice_maximo([5, 6])
    1
    >>> indice_maximo([5, 6, 6])
    1
    >>> indice_maximo([5, 6, 6, 8])
    3
    '''
    assert len(lst) != 0
    imax = 0
    for i in range(1, len(lst)):
        if lst[i] > lst[imax]:
            imax = i
    return imax

# Generated from doctests.
assert indice_maximo([5]) == 0
assert indice_maximo([5, 6]) == 1
assert indice_maximo([5, 6, 6]) == 1
assert indice_maximo([5, 6, 6, 8]) == 3
