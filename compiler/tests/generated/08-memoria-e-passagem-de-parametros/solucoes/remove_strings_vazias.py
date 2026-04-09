def remove_strings_vazias(lst: list[str]):
    '''
    Remove as strings vazias de *lst*.

    Exemplos
    >>> lst = ['']
    >>> remove_strings_vazias(lst)
    >>> lst
    []
    >>> lst = ['', '', '']
    >>> remove_strings_vazias(lst)
    >>> lst
    []
    >>> lst = ['esta', '', '', 'string', '', 'nao', 'tem']
    >>> remove_strings_vazias(lst)
    >>> lst
    ['esta', 'string', 'nao', 'tem']
    '''
    # Todos os elementos nas posições < i não são vazios
    # Todos os elementos nas posições >= i e < j são vazios
    i = 0
    for j in range(len(lst)):
        if lst[j] != '':
            if i != j:
                # move o elemento não vazia lst[j]
                # para a parte com os elementos não vazios.
                lst[i] = lst[j]
                lst[j] = ''
            i += 1

    # remove os vazios que ficaram no final
    while len(lst) != i:
        lst.pop()

# Generated from doctests.
lst = ['']
remove_strings_vazias(lst)
assert lst == []
lst = ['', '', '']
remove_strings_vazias(lst)
assert lst == []
lst = ['esta', '', '', 'string', '', 'nao', 'tem']
remove_strings_vazias(lst)
assert lst == ['esta', 'string', 'nao', 'tem']
