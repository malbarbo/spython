import math

def numero_azulejos(comprimento: float, altura: float) -> int:
    '''
    Calcula o número de azulejos de 0,2mx0,2m necessários para azulejar uma
    parede de tamanho *comprimento* x *altura* (em metros) considerando que
    nenhum azulejo é perdido e que recortes são descartados.

    Exemplos
    >>> # sem recortes
    >>> # 10 (2.0 / 0.2) x 12 (2.4 / 0.2)
    >>> numero_azulejos(2.0, 2.4)
    120
    >>> # com recortes
    >>> # 8 (ceil(1.5 / 0.2)) x 12 (ceil(2.3 / 0.2))
    >>> numero_azulejos(1.5, 2.3)
    96

    Algumas situações particulares
    >>> numero_azulejos(0.2, 0.2)
    1
    >>> numero_azulejos(0.3, 0.2)
    2
    >>> numero_azulejos(0.3, 0.3)
    4
    >>> numero_azulejos(0.4, 0.4)
    4
    '''
    return math.ceil(comprimento / 0.2) * math.ceil(altura / 0.2)

# Generated from doctests.
assert numero_azulejos(2.0, 2.4) == 120
assert numero_azulejos(1.5, 2.3) == 96
assert numero_azulejos(0.2, 0.2) == 1
assert numero_azulejos(0.3, 0.2) == 2
assert numero_azulejos(0.3, 0.3) == 4
assert numero_azulejos(0.4, 0.4) == 4
