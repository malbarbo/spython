def dma_para_amd(data: str) -> str:
    '''
    Transforma *data*, que deve estar no formato "dia/mes/ano",
    onde dia e mes tem dois dígitos e ano tem quatro dígitos,
    para o formato "ano/mes/dia".

    Exemplo
    >>> dma_para_amd('02/07/2022')
    '2022/07/02'
    '''
    return data[6:] + '/' + data[3:5] + '/' + data[:2]

# Generated from doctests.
assert dma_para_amd('02/07/2022') == '2022/07/02'
