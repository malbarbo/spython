def no_edge_spaces(text: str) -> bool:
    '''
    Returns True if *text* does not start or end with spaces.

    Examples
    >>> no_edge_spaces('')
    True
    >>> no_edge_spaces('No spaces here.')
    True
    >>> no_edge_spaces(' starts with space')
    False
    >>> no_edge_spaces('ends with space ')
    False
    >>> no_edge_spaces(' both ')
    False
    '''
    return text == '' or (text[0] != ' ' and text[-1] != ' ')


assert no_edge_spaces('') == True
assert no_edge_spaces('No spaces here.') == True
assert no_edge_spaces(' starts with space') == False
assert no_edge_spaces('ends with space ') == False
assert no_edge_spaces(' both ') == False
