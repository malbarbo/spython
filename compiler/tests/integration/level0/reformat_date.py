def reformat_date(date: str) -> str:
    '''
    Transforms *date* from "day/month/year" format to "year/month/day" format.
    Requires day and month to have two digits and year to have four digits.

    Examples
    >>> reformat_date('02/07/2022')
    '2022/07/02'
    '''
    return date[6:] + '/' + date[3:5] + '/' + date[:2]


assert reformat_date('02/07/2022') == '2022/07/02'
assert reformat_date('31/12/1999') == '1999/12/31'
assert reformat_date('01/01/2000') == '2000/01/01'
