def zera_dezena_e_unidade(n: int) -> int:
    '''
    Zera a dezena e unidade de *n*.

    >>> zera_dezena_e_unidade(19)
    0
    >>> zera_dezena_e_unidade(341)
    300
    >>> zera_dezena_e_unidade(5251)
    5200
    '''
    return n // 100 * 100

# Generated from doctests.
assert zera_dezena_e_unidade(19) == 0
assert zera_dezena_e_unidade(341) == 300
assert zera_dezena_e_unidade(5251) == 5200
