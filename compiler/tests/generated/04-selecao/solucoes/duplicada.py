def duplicada(palavra: str) -> bool:
    '''
    Produz True se *palavra* é duplicada, isto é, é formada pela ocorrência de
    duas partes iguais separadas ou não por hífen. Devolve False caso
    contrário.

    Exemplos:
    >>> duplicada('xixi')
    True
    >>> duplicada("lero-lero")
    True
    >>> duplicada("aba")
    False
    >>> duplicada("ab-ba")
    False
    '''

    m = len(palavra) // 2
    if len(palavra) % 2 == 0:
        r = palavra[:m] == palavra[m:]
    else:
        r = palavra[:m] == palavra[m + 1:] and palavra[m] == '-'
    # O if não é necessário, podemos simplificar para
    # return len(palavra) % 2 == 0 and palavra[:m] == palavra[m:] or \
    #        len(palavra) % 2 == 1 and palavra[:m] == palavra[m + 1:] and palavra[m] == '-'
    return r

# Generated from doctests.
assert duplicada('xixi') == True
assert duplicada("lero-lero") == True
assert duplicada("aba") == False
assert duplicada("ab-ba") == False
