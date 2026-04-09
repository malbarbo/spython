def string_ints(s: str) -> list[int]:
    '''
    Identifica os inteiros que estão em *s*.
    Se *s* tem mais de um inteiro, então eles devem estar
    separados por vírgula, como em '512,12,145'.

    >>> string_ints('')
    []
    >>> string_ints('102')
    [102]
    >>> string_ints('512,12,145')
    [512, 12, 145]
    '''
    ints = []
    while ',' in s:
        i = s.index(',')
        ints.append(int(s[:i]))
        s = s[i + 1:]
    if s != '':
        ints.append(int(s))
    return ints

# Generated from doctests.
assert string_ints('') == []
assert string_ints('102') == [102]
assert string_ints('512,12,145') == [512, 12, 145]
