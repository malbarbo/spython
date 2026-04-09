def insere_pos(lst: list[int], i: int, n: int) -> list[int]:
    '''
    Cria uma nova lista inserindo *n* na posicão *i* de *lst*.

    Requer que 0 <= i <= len(lst).
    >>> insere_pos([], 0, 10)
    [10]
    >>> insere_pos([5], 0, 10)
    [10, 5]
    >>> insere_pos([5], 1, 10)
    [5, 10]
    >>> insere_pos([5, 7], 0, 10)
    [10, 5, 7]
    >>> insere_pos([5, 7], 1, 10)
    [5, 10, 7]
    >>> insere_pos([5, 7], 2, 10)
    [5, 7, 10]
    '''
    assert 0 <= i <= len(lst)

    r = []
    # Copia os elementos de 0 até i
    for j in range(0, i):
        r.append(lst[j])

    # Insere o elemento n na posição i
    r.append(n)

    # Copia os lementos de i até len(lst)
    for j in range(i, len(lst)):
        r.append(lst[j])

    return r

def insere_pos2(lst: list[int], i: int, n: int) -> list[int]:
    '''
    Cria uma nova lista inserindo *n* na posicão *i* de *lst*.

    Requer que 0 <= i <= len(lst).
    >>> insere_pos2([], 0, 10)
    [10]
    >>> insere_pos2([5], 0, 10)
    [10, 5]
    >>> insere_pos2([5], 1, 10)
    [5, 10]
    >>> insere_pos2([5, 7], 0, 10)
    [10, 5, 7]
    >>> insere_pos2([5, 7], 1, 10)
    [5, 10, 7]
    >>> insere_pos2([5, 7], 2, 10)
    [5, 7, 10]
    '''
    assert 0 <= i <= len(lst)
    r = []
    for j in range(len(lst) + 1):
        if j == i:
            r.append(n)
        if j < len(lst):
            r.append(lst[j])
    return r

def insere_pos3(lst: list[int], i: int, n: int) -> list[int]:
    '''
    Cria uma nova lista inserindo *n* na posicão *i* de *lst*.

    Requer que 0 <= i <= len(lst).
    >>> insere_pos3([], 0, 10)
    [10]
    >>> insere_pos3([5], 0, 10)
    [10, 5]
    >>> insere_pos3([5], 1, 10)
    [5, 10]
    >>> insere_pos3([5, 7], 0, 10)
    [10, 5, 7]
    >>> insere_pos3([5, 7], 1, 10)
    [5, 10, 7]
    >>> insere_pos3([5, 7], 2, 10)
    [5, 7, 10]
    '''
    assert 0 <= i <= len(lst)
    return lst[:i] + [n] + lst[i:]

# Generated from doctests.
assert insere_pos([], 0, 10) == [10]
assert insere_pos([5], 0, 10) == [10, 5]
assert insere_pos([5], 1, 10) == [5, 10]
assert insere_pos([5, 7], 0, 10) == [10, 5, 7]
assert insere_pos([5, 7], 1, 10) == [5, 10, 7]
assert insere_pos([5, 7], 2, 10) == [5, 7, 10]
assert insere_pos2([], 0, 10) == [10]
assert insere_pos2([5], 0, 10) == [10, 5]
assert insere_pos2([5], 1, 10) == [5, 10]
assert insere_pos2([5, 7], 0, 10) == [10, 5, 7]
assert insere_pos2([5, 7], 1, 10) == [5, 10, 7]
assert insere_pos2([5, 7], 2, 10) == [5, 7, 10]
assert insere_pos3([], 0, 10) == [10]
assert insere_pos3([5], 0, 10) == [10, 5]
assert insere_pos3([5], 1, 10) == [5, 10]
assert insere_pos3([5, 7], 0, 10) == [10, 5, 7]
assert insere_pos3([5, 7], 1, 10) == [5, 10, 7]
assert insere_pos3([5, 7], 2, 10) == [5, 7, 10]
