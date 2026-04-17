def add_period(text: str) -> str:
    '''
    Returns *text* if it is empty or already ends with a period;
    otherwise returns *text* concatenated with '.'.

    Examples
    >>> add_period('')
    ''
    >>> add_period('Maybe.')
    'Maybe.'
    >>> add_period('Yes, I would like')
    'Yes, I would like.'
    '''
    if text == '':
        with_period = ''
    elif text[len(text) - 1] == '.':
        with_period = text
    else:
        with_period = text + '.'
    return with_period


assert add_period('') == ''
assert add_period('Maybe.') == 'Maybe.'
assert add_period('Yes, I would like') == 'Yes, I would like.'
