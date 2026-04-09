def numero_digitos(n: int) -> int:
    '''
    Calcula o número de dígitos de *n*.
    Se *n* for 0, devolve 1.
    Se *n* for negativo, devolve a quantidade de dígitos
    do valor absoluto de *n*.

    Exemplos
    >>> numero_digitos(0)
    1
    >>> numero_digitos(1231)
    4
    >>> numero_digitos(-45)
    2
    '''
    return len(str(abs(n)))

# Generated from doctests.
assert numero_digitos(0) == 1
assert numero_digitos(1231) == 4
assert numero_digitos(-45) == 2
