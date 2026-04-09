def soma_naturais(n: int) -> int:
    '''
    Soma todos os número naturais menores ou iguais que *n*.

    Requer que n >= 0.

    Exemplos
    >>> soma_naturais(0)
    0
    >>> soma_naturais(1)
    1
    >>> soma_naturais(2)
    3
    >>> soma_naturais(3)
    6
    >>> soma_naturais(4)
    10
    '''
    if n == 0:
        soma = 0
    else:
        soma = n + soma_naturais(n - 1)
    return soma

# Generated from doctests.
assert soma_naturais(0) == 0
assert soma_naturais(1) == 1
assert soma_naturais(2) == 3
assert soma_naturais(3) == 6
assert soma_naturais(4) == 10
