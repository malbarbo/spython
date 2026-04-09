def concatena(lst: list[str]) -> str:
    '''
    Concatena todos os elementos de *lst*.

    Exemplos
    >>> concatena([])
    ''
    >>> concatena(['cc'])
    'cc'
    >>> concatena(['cc', ' é ', 'ciência da computação'])
    'cc é ciência da computação'
    '''
    if lst == []:
        s = ''
    else:
        s = lst[0] + concatena(lst[1:])
    return s

# Generated from doctests.
assert concatena([]) == ''
assert concatena(['cc']) == 'cc'
assert concatena(['cc', ' é ', 'ciência da computação']) == 'cc é ciência da computação'
