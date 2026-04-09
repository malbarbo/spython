def sinal(n: int) -> int:
    '''
    Determina o sinal de *n*.
    Se n < 0, produz -1.
    Se n == 0, produz 0.
    Se n > 0, produz 1.

    Exemplos
    >>> sinal(-6)
    -1
    >>> sinal(0)
    0
    >>> sinal(10)
    1
    '''
    if n < 0:
        s = -1
    elif n == 0:
        s = 0
    else:
        s = 1
    return s

# Generated from doctests.
assert sinal(-6) == -1
assert sinal(0) == 0
assert sinal(10) == 1
