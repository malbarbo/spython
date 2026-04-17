def doubled_word(word: str) -> bool:
    '''
    Returns True if *word* is doubled, that is, formed by two equal halves
    optionally separated by a hyphen.

    Examples
    >>> doubled_word('xixi')
    True
    >>> doubled_word('lero-lero')
    True
    >>> doubled_word('aba')
    False
    >>> doubled_word('ab-ba')
    False
    '''
    m = len(word) // 2
    if len(word) % 2 == 0:
        r = word[:m] == word[m:]
    else:
        r = word[:m] == word[m + 1:] and word[m] == '-'
    return r


assert doubled_word('xixi') == True
assert doubled_word('lero-lero') == True
assert doubled_word('aba') == False
assert doubled_word('ab-ba') == False
assert doubled_word('aa') == True
assert doubled_word('ab') == False
