def concatenate(lst: list[str]) -> str:
    '''
    Concatenates all strings in *lst*.

    Examples
    >>> concatenate([])
    ''
    >>> concatenate(['cc'])
    'cc'
    >>> concatenate(['cc', ' is ', 'computer science'])
    'cc is computer science'
    '''
    if lst == []:
        s = ''
    else:
        s = lst[0] + concatenate(lst[1:])
    return s


assert concatenate([]) == ''
assert concatenate(['cc']) == 'cc'
assert concatenate(['cc', ' is ', 'computer science']) == 'cc is computer science'
