from enum import Enum, auto

class Combustivel(Enum):
    '''O tipo do combustivel em um abastecimento'''
    ALCOOL = auto()
    GASOLINA = auto()

def indica_combustivel(preco_alcool: float, preco_gasolina: float) -> Combustivel:
    '''
    Indica o combustível que deve ser utilizado no abastecimento. Produz
    'alcool' se *preco_alcool* for menor ou igual a 70% do *preco_gasolina*,
    caso contrário produz 'gasolina'.

    Exemplos
    >>> # 'alcool'
    >>> # preco_alcool <= 0.7 * preco_gasolina é True
    >>> indica_combustivel(4.00, 6.00).name
    'ALCOOL'
    >>> indica_combustivel(3.50, 5.00).name
    'ALCOOL'
    >>> # 'gasolina'
    >>> # preco_alcool <= 0.7 * preco_gasolina é False
    >>> indica_combustivel(4.00, 5.00).name
    'GASOLINA'
    '''
    if preco_alcool <= 0.7 * preco_gasolina:
        combustivel = Combustivel.ALCOOL
    else:
        combustivel = Combustivel.GASOLINA
    return combustivel

# Generated from doctests.
assert indica_combustivel(4.00, 6.00).name == 'ALCOOL'
assert indica_combustivel(3.50, 5.00).name == 'ALCOOL'
assert indica_combustivel(4.00, 5.00).name == 'GASOLINA'
