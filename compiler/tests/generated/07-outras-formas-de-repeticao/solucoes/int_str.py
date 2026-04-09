def int_str(n: int) -> str:
    '''
    Converte *n* para uma string.
    Requer que n >= 0.

    Exemplos
    >>> int_str(0)
    '0'
    >>> int_str(9)
    '9'
    >>> int_str(10)
    '10'
    >>> int_str(71620)
    '71620'
    >>> int_str(123456)
    '123456'
    '''
    assert n >= 0
    digitos = '0123456789'
    if n == 0:
        r = '0'
    else:
        r = ''
        while n > 0:
            r = digitos[n % 10] + r
            n = n // 10
    return r

# Generated from doctests.
assert int_str(0) == '0'
assert int_str(9) == '9'
assert int_str(10) == '10'
assert int_str(71620) == '71620'
assert int_str(123456) == '123456'
