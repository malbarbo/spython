def aumenta(valor: float, porcentagem: float) -> float:
    '''
    Aumenta *valor* pela *porcentagem*.

    Exemplos
    >>> aumenta(100.0, 3.0)
    103.0
    >>> aumenta(20.0, 50.0)
    30.0
    >>> aumenta(10.0, 80.0)
    18.0
    '''
    return valor + porcentagem / 100.0 * valor

# Generated from doctests.
assert aumenta(100.0, 3.0) == 103.0
assert aumenta(20.0, 50.0) == 30.0
assert aumenta(10.0, 80.0) == 18.0
