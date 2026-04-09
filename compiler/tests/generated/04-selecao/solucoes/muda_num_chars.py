def muda_num_chars(s: str, n: int) -> str:
    '''
    Cria uma nova string com *n* caracteres a partir de *s*.
    Se *s* tem menos que *n* caracteres, adiciona espaços no final.
    Se *s* tem mais que *n* caracteres, remove os caracteres excedentes do final.
    Se *s* tem *n* caracteres, devolve *s*.

    Requer que n >= 0.

    Exemplos
    >>> muda_num_chars('casa', 7)
    'casa   '
    >>> muda_num_chars('computação', 4)
    'comp'
    >>> muda_num_chars('python', 6)
    'python'
    '''
    assert n >= 0
    if len(s) < n:
        r = s + ' ' * (n - len(s))
    elif len(s) > n:
        r = s[:n]
    else:
        r = s
    return r

# Generated from doctests.
assert muda_num_chars('casa', 7) == 'casa   '
assert muda_num_chars('computação', 4) == 'comp'
assert muda_num_chars('python', 6) == 'python'
