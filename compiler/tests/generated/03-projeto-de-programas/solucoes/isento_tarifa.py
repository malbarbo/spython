def isento_tarifa(idade: int) -> bool:
    '''
    Produz True se uma pessoa de *idade* anos é isento da tarifa de transporte
    público, isto é, tem menos que 18 anos ou 65 ou mais. Produz False caso
    contrário.

    Exemplos
    >>> isento_tarifa(17)
    True
    >>> isento_tarifa(18)
    False
    >>> isento_tarifa(50)
    False
    >>> isento_tarifa(65)
    True
    >>> isento_tarifa(70)
    True
    '''
    return idade < 18 or idade >= 65

# Generated from doctests.
assert isento_tarifa(17) == True
assert isento_tarifa(18) == False
assert isento_tarifa(50) == False
assert isento_tarifa(65) == True
assert isento_tarifa(70) == True
