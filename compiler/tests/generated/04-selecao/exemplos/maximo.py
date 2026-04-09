def maximo(a: int, b: int) -> int:
    '''
    Devolve o valor máximo entre *a* e *b*.

    Exemplos
    >>> # a é o máximo
    >>> maximo(10, 8)
    10
    >>> # b é o máximo
    >>> maximo(-2, -1)
    -1
    >>> maximo(6, 6)
    6
    '''
    if a > b:
        m = a
    else:
        m = b
    return m

def maximo3(a: int, b: int, c: int) -> int:
    '''
    Encontra o valor máximo entre *a*, *b* e *c*.

    Exemplos

    >>> # a é o máximo
    >>> maximo3(20, 10, 12)
    20
    >>> maximo3(20, 12, 10)
    20
    >>> maximo3(20, 12, 12)
    20
    >>> maximo3(20, 20, 20)
    20
    >>> # b é o máximo
    >>> maximo3(5, 12, 3)
    12
    >>> maximo3(3, 12, 5)
    12
    >>> maximo3(5, 12, 5)
    12
    >>> # c é o máximo
    >>> maximo3(4, 8, 18)
    18
    >>> maximo3(8, 4, 18)
    18
    >>> maximo3(8, 8, 18)
    18
    '''
    return maximo(maximo(a, b), c)

# Generated from doctests.
assert maximo(10, 8) == 10
assert maximo(-2, -1) == -1
assert maximo(6, 6) == 6
assert maximo3(20, 10, 12) == 20
assert maximo3(20, 12, 10) == 20
assert maximo3(20, 12, 12) == 20
assert maximo3(20, 20, 20) == 20
assert maximo3(5, 12, 3) == 12
assert maximo3(3, 12, 5) == 12
assert maximo3(5, 12, 5) == 12
assert maximo3(4, 8, 18) == 18
assert maximo3(8, 4, 18) == 18
assert maximo3(8, 8, 18) == 18
