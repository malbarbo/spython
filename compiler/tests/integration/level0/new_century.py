def new_century(date: str) -> bool:
    '''
    Checks if *date* represents the first day of a century, that is,
    January 1st of a year ending in 00.

    Examples
    >>> new_century('01/01/1900')
    True
    >>> new_century('01/01/2000')
    True
    >>> new_century('03/01/2100')
    False
    >>> new_century('01/02/2000')
    False
    >>> new_century('01/01/1230')
    False
    '''
    day = date[:2]
    month = date[3:5]
    decade = date[8:]
    return day == '01' and month == '01' and decade == '00'


assert new_century('01/01/1900') == True
assert new_century('01/01/2000') == True
assert new_century('03/01/2100') == False
assert new_century('01/02/2000') == False
assert new_century('01/01/1230') == False
