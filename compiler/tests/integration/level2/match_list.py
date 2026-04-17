def describe_list(lst: list[int]) -> str:
    '''
    Describes the structure of *lst*.

    Examples
    >>> describe_list([])
    'empty'
    >>> describe_list([7])
    'singleton: 7'
    >>> describe_list([3, 4])
    'pair: 3 and 4'
    >>> describe_list([1, 2, 3])
    'longer'
    '''
    match lst:
        case []:
            desc = 'empty'
        case [x]:
            desc = 'singleton: ' + str(x)
        case [x, y]:
            desc = 'pair: ' + str(x) + ' and ' + str(y)
        case _:
            desc = 'longer'
    return desc


def head_or_default(lst: list[int], default: int) -> int:
    '''
    Returns the first element of *lst*, or *default* if empty.

    Examples
    >>> head_or_default([], 0)
    0
    >>> head_or_default([5], 0)
    5
    >>> head_or_default([3, 7, 1], 0)
    3
    '''
    match lst:
        case []:
            result = default
        case [x, *_]:
            result = x
    return result


assert describe_list([]) == 'empty'
assert describe_list([7]) == 'singleton: 7'
assert describe_list([3, 4]) == 'pair: 3 and 4'
assert describe_list([1, 2, 3]) == 'longer'
assert head_or_default([], 0) == 0
assert head_or_default([5], 0) == 5
assert head_or_default([3, 7, 1], 0) == 3
