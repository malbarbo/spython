def indica_combustivel(preco_alcool: float, preco_gasolina: float) -> str:
    '''
    Indica o combustível que deve ser utilizado no abastecimento. Produz
    'alcool' se *preco_alcool* for menor ou igual a 70% do *preco_gasolina*,
    caso contrário produz 'gasolina'.

    Exemplos
    >>> # 'alcool'
    >>> indica_combustivel(4.00, 6.00) # 4.00 <= 0.7 * 6.00 é True
    'alcool'
    >>> indica_combustivel(3.50, 5.00) # 3.50 <= 0.7 * 5.00 é True
    'alcool'
    >>> # 'gasolina'
    >>> indica_combustivel(4.00, 5.00) # 4.00 <= 0.7 * 5.00 é False
    'gasolina'
    '''
    if preco_alcool <= 0.7 * preco_gasolina:
        combustivel = 'alcool'
    else:
        combustivel = 'gasolina'
    return combustivel

# Generated from doctests.
assert indica_combustivel(4.00, 6.00) == 'alcool'
assert indica_combustivel(3.50, 5.00) == 'alcool'
assert indica_combustivel(4.00, 5.00) == 'gasolina'
