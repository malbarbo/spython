def freq(v: int, lst: list[int]) -> int:
    '''
    Conta quantas vezes *v* aparece
    em *lst*.

    Exemplos
    >>> freq(1, [])
    0
    >>> freq(1, [7])
    0
    >>> freq(1, [1, 7, 1])
    2
    >>> freq(4, [4, 1, 7, 4, 4])
    3
    '''
    if lst == []:
        cont = 0
    else:
        if v == lst[0]:
            cont = 1 + freq(v, lst[1:])
        else:
            cont = freq(v, lst[1:])
    return cont

# Generated from doctests.
assert freq(1, []) == 0
assert freq(1, [7]) == 0
assert freq(1, [1, 7, 1]) == 2
assert freq(4, [4, 1, 7, 4, 4]) == 3
