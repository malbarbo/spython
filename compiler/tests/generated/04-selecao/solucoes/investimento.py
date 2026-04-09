def rendimento1(valor: float) -> float:
    '''
    Determina quanto o investimento de *valor* renderá em um ano considerando
    as seguintes taxas:

    - valor <= 2000, 10%
    - 2000 < valor <= 5000, 12%
    - valor > 5000, 13%

    Exemplos
    >>> rendimento1(1000.0)
    100.0
    >>> rendimento1(2000.0)
    200.0
    >>> rendimento1(4000.0)
    480.0
    >>> rendimento1(5000.0)
    600.0
    >>> rendimento1(6000.0)
    780.0
    '''
    if valor <= 2000.0:
        r = valor * 0.1
    elif valor <= 5000.0:
        r = valor * 0.12
    else:
        r = valor * 0.13
    return r

def rendimento2(valor: float) -> float:
    '''
    Determina quanto o investimento de *valor* renderá em dois ano considerando
    as seguintes taxas por ano:

    - valor <= 2000, 10%
    - 2000 < valor <= 5000, 12%
    - valor > 5000, 13%

    Exemplos
    >>> rendimento2(1000.0)
    110.0
    >>> rendimento2(2000.0)
    264.0
    >>> rendimento2(4000.0)
    537.6
    >>> rendimento2(5000.0)
    728.0
    >>> rendimento2(6000.0)
    881.4
    '''
    return rendimento1(valor + rendimento1(valor))

# Generated from doctests.
assert rendimento1(1000.0) == 100.0
assert rendimento1(2000.0) == 200.0
assert rendimento1(4000.0) == 480.0
assert rendimento1(5000.0) == 600.0
assert rendimento1(6000.0) == 780.0
assert rendimento2(1000.0) == 110.0
assert rendimento2(2000.0) == 264.0
assert rendimento2(4000.0) == 537.6
assert rendimento2(5000.0) == 728.0
assert rendimento2(6000.0) == 881.4
