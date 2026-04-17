def hit_count(bet: list[int], drawn: list[int]) -> int:
    '''
    Counts how many numbers in *bet* appear in *drawn*.

    Examples
    >>> hit_count([1, 2, 3, 4, 5, 6], [8, 12, 20, 41, 52, 57])
    0
    >>> hit_count([8, 2, 3, 4, 5, 6], [8, 12, 20, 41, 52, 57])
    1
    >>> hit_count([8, 12, 20, 41, 52, 57], [8, 12, 20, 41, 52, 57])
    6
    '''
    hits = 0

    for n in bet:
        if is_drawn(n, drawn):
            hits = hits + 1

    return hits


def is_drawn(n: int, drawn: list[int]) -> bool:
    '''
    Returns True if *n* is one of the numbers in *drawn*.

    Examples
    >>> is_drawn(1, [1, 7, 10, 40, 41, 60])
    True
    >>> is_drawn(2, [1, 7, 10, 40, 41, 60])
    False
    '''
    in_drawn = False

    for x in drawn:
        if n == x:
            in_drawn = True

    return in_drawn


assert hit_count([1, 2, 3, 4, 5, 6], [8, 12, 20, 41, 52, 57]) == 0
assert hit_count([8, 2, 3, 4, 5, 6], [8, 12, 20, 41, 52, 57]) == 1
assert hit_count([8, 12, 3, 4, 5, 6], [8, 12, 20, 41, 52, 57]) == 2
assert hit_count([8, 12, 20, 4, 5, 6], [8, 12, 20, 41, 52, 57]) == 3
assert hit_count([8, 12, 20, 41, 5, 6], [8, 12, 20, 41, 52, 57]) == 4
assert hit_count([8, 12, 20, 41, 52, 6], [8, 12, 20, 41, 52, 57]) == 5
assert hit_count([8, 12, 20, 41, 52, 57], [8, 12, 20, 41, 52, 57]) == 6
_drawn = [1, 7, 10, 40, 41, 60]
assert is_drawn(1, _drawn) == True
assert is_drawn(7, _drawn) == True
assert is_drawn(10, _drawn) == True
assert is_drawn(40, _drawn) == True
assert is_drawn(41, _drawn) == True
assert is_drawn(60, _drawn) == True
assert is_drawn(2, _drawn) == False
