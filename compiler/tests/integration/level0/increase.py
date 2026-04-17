def increase(value: float, percentage: float) -> float:
    '''
    Increases *value* by *percentage*.

    Examples
    >>> increase(100.0, 3.0)
    103.0
    >>> increase(20.0, 50.0)
    30.0
    >>> increase(10.0, 80.0)
    18.0
    '''
    return value + percentage / 100.0 * value


assert increase(100.0, 3.0) == 103.0
assert increase(20.0, 50.0) == 30.0
assert increase(10.0, 80.0) == 18.0
