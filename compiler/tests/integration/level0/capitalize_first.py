def capitalize_first(phrase: str) -> str:
    '''
    Returns *phrase* with only the first letter capitalized.
    Requires *phrase* to start with a letter.

    Examples
    >>> capitalize_first('john won.')
    'John won.'
    >>> capitalize_first('HELLO WORLD.')
    'Hello world.'
    '''
    return phrase[0].upper() + phrase[1:].lower()


assert capitalize_first('john won.') == 'John won.'
assert capitalize_first('HELLO WORLD.') == 'Hello world.'
