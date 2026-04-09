def novo_seculo(data: str) -> bool:
    '''
    Verifica se *data* representa o primeiro dia de um século, isto
    é, primeiro de janeiro de um ano que termina com 00.

    Exemplos
    >>> novo_seculo('01/01/1900')
    True
    >>> novo_seculo('01/01/2000')
    True
    >>> novo_seculo('03/01/2100')
    False
    >>> novo_seculo('01/02/2000')
    False
    >>> novo_seculo('01/01/1230')
    False
    '''
    dia = data[:2]
    mes = data[3:5]
    decada = data[8:]
    return dia == '01' and mes == '01' and decada == '00'

# Generated from doctests.
assert novo_seculo('01/01/1900') == True
assert novo_seculo('01/01/2000') == True
assert novo_seculo('03/01/2100') == False
assert novo_seculo('01/02/2000') == False
assert novo_seculo('01/01/1230') == False
