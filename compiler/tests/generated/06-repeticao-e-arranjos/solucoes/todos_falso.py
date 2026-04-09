def todos_false(lst: list[bool]) -> bool:
    '''
    Produz True se todos os elemento de *lst* são False, produz False caso contrário.
    Exemplos
    >>> todos_false([])
    True
    >>> todos_false([False])
    True
    >>> todos_false([False, True, False])
    False
    '''
    todos_false = True
    for b in lst:
        if b:
            todos_false = False
    return todos_false

# Generated from doctests.
assert todos_false([]) == True
assert todos_false([False]) == True
assert todos_false([False, True, False]) == False
