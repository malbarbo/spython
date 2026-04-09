def soma1(lst: list[int], n: int):
    '''
    Soma 1 em cada elemento de *lst[:n]*

    Requer que 0 <= n <= len(lst).

    Exemplos:
    >>> lst = [5, 1, 4]
    >>> soma1(lst, 3)
    >>> lst
    [6, 2, 5]
    >>> soma1(lst, 2)
    >>> lst
    [7, 3, 5]
    >>> soma1(lst, 1)
    >>> lst
    [8, 3, 5]
    >>> soma1(lst, 0)
    >>> lst
    [8, 3, 5]
    '''
    if n > 0:
        lst[n - 1] = lst[n - 1] + 1
        soma1(lst, n - 1)

# Generated from doctests.
lst = [5, 1, 4]
soma1(lst, 3)
assert lst == [6, 2, 5]
soma1(lst, 2)
assert lst == [7, 3, 5]
soma1(lst, 1)
assert lst == [8, 3, 5]
soma1(lst, 0)
assert lst == [8, 3, 5]
