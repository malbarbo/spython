def soma(lst: list[int]) -> int:
    '''
    Soma os elementos de *lst*.
    Exemplos
    >>> soma([])
    0
    >>> soma([3])
    3
    >>> soma([3, 7])
    10
    >>> soma([3, 7, 2])
    12
    '''
    soma = 0
    for n in lst:
        soma = soma + n
    return soma

# Generated from doctests.
assert soma([]) == 0
assert soma([3]) == 3
assert soma([3, 7]) == 10
assert soma([3, 7, 2]) == 12
