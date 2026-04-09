def media_tamanho(lst: list[str]) -> float:
    '''
    Calcula a média dos tamanhos das strings de *lst*.
    Requer que *lst* seja não vazia.

    Exemplos
    >>> media_tamanho(['casa'])
    4.0
    >>> media_tamanho(['casa', 'da'])
    3.0
    >>> media_tamanho(['casa', 'da', ''])
    2.0
    >>> media_tamanho(['casa', 'da', '', 'onça'])
    2.5
    '''
    quant = 0
    media = 0.0
    for s in lst:
        media = (quant * media + len(s)) / (quant + 1)
        quant = quant + 1
    return media

def media_tamanho2(lst: list[str]) -> float:
    '''
    Calcula a média dos tamanhos das strings de *lst*.
    Requer que *lst* seja não vazia.

    Exemplos
    >>> media_tamanho2(['casa'])
    4.0
    >>> media_tamanho2(['casa', 'da'])
    3.0
    >>> media_tamanho2(['casa', 'da', ''])
    2.0
    >>> media_tamanho2(['casa', 'da', '', 'onça'])
    2.5
    '''
    assert len(lst) != 0

    # Soma dos tamanhos
    soma = 0
    for s in lst:
        soma = soma + len(s)

    # Média
    return soma / len(lst)

# Generated from doctests.
assert media_tamanho(['casa']) == 4.0
assert media_tamanho(['casa', 'da']) == 3.0
assert media_tamanho(['casa', 'da', '']) == 2.0
assert media_tamanho(['casa', 'da', '', 'onça']) == 2.5
assert media_tamanho2(['casa']) == 4.0
assert media_tamanho2(['casa', 'da']) == 3.0
assert media_tamanho2(['casa', 'da', '']) == 2.0
assert media_tamanho2(['casa', 'da', '', 'onça']) == 2.5
