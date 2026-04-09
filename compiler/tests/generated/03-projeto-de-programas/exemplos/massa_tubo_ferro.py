PI: float = 3.14

DENSIDADE_FERRO: float = 7874

def massa_tubo_ferro(diametro_externo: float, diametro_interno: float, altura: float) -> float:
    '''
    Calcula a massa de um tubo de ferro a partir das suas dimensões.

    Requer diametro_externo > diametro_interno.

    Exemplos
    >>> # 3.14 * ((0.05 / 2) ** 2 - (0.03 / 2) ** 2) * 0.1 * 7874
    >>> round(massa_tubo_ferro(0.05, 0.03, 0.1), 7)
    0.9889744
    '''
    area_externa = PI * (diametro_externo / 2) ** 2
    area_interna = PI * (diametro_interno / 2) ** 2
    volume = (area_externa - area_interna) * altura
    return volume * DENSIDADE_FERRO

# Generated from doctests.
assert round(massa_tubo_ferro(0.05, 0.03, 0.1), 7) == 0.9889744
