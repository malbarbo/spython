def soma(lst: list[int]) -> int:
    '''
    Soma os elementos de *lst*.
    Exemplos
    >>> soma([])
    0
    >>> soma([6])
    6
    >>> soma([3, 6])
    9
    >>> soma([7, 3, 6])
    16
    '''
    if lst == []:
        s = 0
    else:
        s = lst[0] + soma(lst[1:])
    return s

def soma_inc(lst: list[int], i: int) -> int:
    '''
    Soma os elementos de *lst* a partir de *i*, isto é,
    soma os elementos de *lst[i:]*.
    Requer que 0 <= i <= len(lst)
    >>> soma_inc([7, 3, 6], 0)
    16
    >>> soma_inc([7, 3, 6], 1)
    9
    >>> soma_inc([7, 3, 6], 2)
    6
    >>> soma_inc([7, 3, 6], 3)
    0
    '''
    if i >= len(lst):
        s = 0
    else:
        s = lst[i] + soma_inc(lst, i + 1)
    return s

def soma_dec(lst: list[int], i: int) -> int:
    '''
    Soma os primeiro *i* elementos de *lst*, isto é,
    soma os elementos de *lst[:i]*.
    Requer que 0 <= i <= len(lst)
    >>> soma_dec([7, 3, 6], 0)
    0
    >>> soma_dec([7, 3, 6], 1)
    7
    >>> soma_dec([7, 3, 6], 2)
    10
    >>> soma_dec([7, 3, 6], 3)
    16
    '''
    if i <= 0:
        s = 0
    else:
        s = lst[i - 1] + soma_dec(lst, i - 1)
    return s

# Generated from doctests.
assert soma([]) == 0
assert soma([6]) == 6
assert soma([3, 6]) == 9
assert soma([7, 3, 6]) == 16
assert soma_inc([7, 3, 6], 0) == 16
assert soma_inc([7, 3, 6], 1) == 9
assert soma_inc([7, 3, 6], 2) == 6
assert soma_inc([7, 3, 6], 3) == 0
assert soma_dec([7, 3, 6], 0) == 0
assert soma_dec([7, 3, 6], 1) == 7
assert soma_dec([7, 3, 6], 2) == 10
assert soma_dec([7, 3, 6], 3) == 16
