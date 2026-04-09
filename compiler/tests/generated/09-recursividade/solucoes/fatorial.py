def fatorial(n: int) -> int:
    '''
    Calcula o fatorial de *n*, isto é, o produto dos *n* primeiros números
    naturais maiores que 0.

    Requer que n >= 0.

    Exemplos
    >>> fatorial(0)
    1
    >>> fatorial(1)
    1
    >>> fatorial(2)
    2
    >>> fatorial(3)
    6
    >>> fatorial(4)
    24
    '''
    assert n >= 0
    if n == 0:
        fat = 1
    else:
        fat = n * fatorial(n - 1)
    return fat

# Generated from doctests.
assert fatorial(0) == 1
assert fatorial(1) == 1
assert fatorial(2) == 2
assert fatorial(3) == 6
assert fatorial(4) == 24
