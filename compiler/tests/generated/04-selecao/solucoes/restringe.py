def restringe(n: int, minimo: int, maximo: int) -> int:
    '''
    Restringe o valor de *n* para o intervalo [*minimo*, *maximo*].

    Se *n* já está no intervalo, então devolve *n*.
    Se *n* < *minimo*, devolve *minimo*.
    Se *n* > *maximo*, devolve *maximo*.

    Exemplos
    >>> restringe(1, 2, 10)
    2
    >>> restringe(2, 2, 10)
    2
    >>> restringe(5, 2, 10)
    5
    >>> restringe(10, 2, 10)
    10
    >>> restringe(14, 2, 10)
    10
    '''
    if n < minimo:
        r = minimo
    elif maximo < n:
        r = maximo
    else:
        r = n
    return r

def restringe_alt(n: int, minimo: int, maximo: int) -> int:
    '''
    Restringe o valor de *n* para o intervalo [*minimo*, *maximo*].

    Se *n* já está no intervalo, então devolve *n*.
    Se *n* < *minimo*, devolve *minimo*.
    Se *n* > *maximo*, devolve *maximo*.

    Exemplos
    >>> restringe_alt(1, 2, 10)
    2
    >>> restringe_alt(2, 2, 10)
    2
    >>> restringe_alt(5, 2, 10)
    5
    >>> restringe_alt(10, 2, 10)
    10
    >>> restringe_alt(14, 2, 10)
    10
    '''
    return max(minimo, min(n, maximo))

# Generated from doctests.
assert restringe(1, 2, 10) == 2
assert restringe(2, 2, 10) == 2
assert restringe(5, 2, 10) == 5
assert restringe(10, 2, 10) == 10
assert restringe(14, 2, 10) == 10
assert restringe_alt(1, 2, 10) == 2
assert restringe_alt(2, 2, 10) == 2
assert restringe_alt(5, 2, 10) == 5
assert restringe_alt(10, 2, 10) == 10
assert restringe_alt(14, 2, 10) == 10
