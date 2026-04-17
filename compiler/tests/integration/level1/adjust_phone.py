def adjust_phone(number: str) -> str:
    '''
    Adjusts *number* by inserting 9 as the ninth digit if needed, that is,
    if *number* has only 8 digits (not counting the area code).

    Requires number to be in format (XX) XXXX-XXXX or (XX) XXXXX-XXXX,
    where X can be any digit.

    Examples
    >>> # no adjustment needed
    >>> adjust_phone('(51) 95872-9989')
    '(51) 95872-9989'
    >>> # '(44) 9787-1241'[:5] + '9' + '(44) 9787-1241'[5:]
    >>> adjust_phone('(44) 9787-1241')
    '(44) 99787-1241'
    '''
    if len(number) == 15:
        adjusted = number
    else:
        adjusted = number[:5] + '9' + number[5:]
    return adjusted


assert adjust_phone('(51) 95872-9989') == '(51) 95872-9989'
assert adjust_phone('(44) 9787-1241') == '(44) 99787-1241'
