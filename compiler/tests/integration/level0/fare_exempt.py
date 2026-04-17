def fare_exempt(age: int) -> bool:
    '''
    Returns True if a person of *age* years is exempt from the public transport fare,
    that is, they are under 18 or 65 and older.

    Examples
    >>> fare_exempt(17)
    True
    >>> fare_exempt(18)
    False
    >>> fare_exempt(50)
    False
    >>> fare_exempt(65)
    True
    >>> fare_exempt(70)
    True
    '''
    return age < 18 or age >= 65


assert fare_exempt(17) == True
assert fare_exempt(18) == False
assert fare_exempt(50) == False
assert fare_exempt(65) == True
assert fare_exempt(70) == True
