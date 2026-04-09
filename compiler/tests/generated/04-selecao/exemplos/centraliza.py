def centraliza(s: str, n: int) -> str:
    '''
    Produz uma string adicionando espaços no início e fim de *s*, se
    necessário, de modo que ela fique com *n* caracteres.

    Se *s* tem mais que *n* caracteres, devolve *s*.

    A quantidade de espaços adicionado no início é igual ou um a mais do que a
    quantidade adicionada no fim.

    Exemplos
    >>> centraliza('casa', 3)
    'casa'
    >>> centraliza('', 0)
    ''
    >>> centraliza('casa', 10)
    '   casa   '
    >>> centraliza('casa', 9)
    '   casa  '
    >>> centraliza('apenas', 10)
    '  apenas  '
    >>> centraliza('apenas', 9)
    '  apenas '
    '''
    if len(s) >= n:
        r = s
    else:
        faltando = n - len(s)
        fim = faltando // 2
        inicio = faltando - fim
        r = ' ' * inicio + s + ' ' * fim
    return r

# Generated from doctests.
assert centraliza('casa', 3) == 'casa'
assert centraliza('', 0) == ''
assert centraliza('casa', 10) == '   casa   '
assert centraliza('casa', 9) == '   casa  '
assert centraliza('apenas', 10) == '  apenas  '
assert centraliza('apenas', 9) == '  apenas '
