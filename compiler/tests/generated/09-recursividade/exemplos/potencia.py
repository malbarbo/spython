def potencia(a: float, n: int) -> float:
    '''
    Calcula *a* elevado a *n*.
    Requer que a != 0 e n >= 0.

    Exemplos
    >>> potencia(2.0, 0)
    1.0
    >>> potencia(2.0, 1)
    2.0
    >>> potencia(2.0, 2)
    4.0
    >>> potencia(2.0, 3)
    8.0
    >>> potencia(3.0, 3)
    27.0
    >>> potencia(3.0, 4)
    81.0
    '''
    if n == 0:
        an = 1.0
    else:
        an = a * potencia(a, n - 1)
    return an

# Generated from doctests.
assert potencia(2.0, 0) == 1.0
assert potencia(2.0, 1) == 2.0
assert potencia(2.0, 2) == 4.0
assert potencia(2.0, 3) == 8.0
assert potencia(3.0, 3) == 27.0
assert potencia(3.0, 4) == 81.0
