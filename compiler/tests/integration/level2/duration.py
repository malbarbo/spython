from dataclasses import dataclass


@dataclass
class Duration:
    '''
    Represents a time duration. hours, minutes, and seconds must be
    non-negative. minutes and seconds must be less than 60.
    '''
    hours: int
    minutes: int
    seconds: int


def seconds_to_duration(secs: int) -> Duration:
    '''
    Converts *secs* seconds into an equivalent Duration in hours, minutes,
    and seconds. The minutes and seconds in the result are always less than 60.

    Requires secs to be non-negative.

    Examples
    >>> # 160 // 60 -> 2 mins, 160 % 60 -> 40 secs
    >>> seconds_to_duration(160)
    Duration(hours=0, minutes=2, seconds=40)
    >>> # 3760 // 3600 -> 1 hour; 3760 % 3600 -> 160 remaining seconds
    >>> # 160 // 60 -> 2 mins, 160 % 60 -> 40 secs
    >>> seconds_to_duration(3760)
    Duration(hours=1, minutes=2, seconds=40)
    '''
    h = secs // 3600
    remaining = secs % 3600
    m = remaining // 60
    s = remaining % 60
    return Duration(h, m, s)


def duration_to_string(t: Duration) -> str:
    '''
    Converts *t* into a human-readable string. Each non-zero component
    appears with its unit. Components are separated by ',' and 'and'
    following English grammar. If *t* is Duration(0, 0, 0), returns
    '0 second(s)'.

    Examples
    >>> duration_to_string(Duration(0, 0, 0))
    '0 second(s)'
    >>> duration_to_string(Duration(0, 0, 1))
    '1 second(s)'
    >>> duration_to_string(Duration(0, 0, 10))
    '10 second(s)'
    >>> duration_to_string(Duration(0, 1, 20))
    '1 minute(s) and 20 second(s)'
    >>> duration_to_string(Duration(0, 2, 0))
    '2 minute(s)'
    >>> duration_to_string(Duration(1, 2, 1))
    '1 hour(s), 2 minute(s) and 1 second(s)'
    >>> duration_to_string(Duration(4, 0, 25))
    '4 hour(s) and 25 second(s)'
    >>> duration_to_string(Duration(2, 4, 0))
    '2 hour(s) and 4 minute(s)'
    >>> duration_to_string(Duration(3, 0, 0))
    '3 hour(s)'
    '''
    h = str(t.hours) + ' hour(s)'
    m = str(t.minutes) + ' minute(s)'
    s = str(t.seconds) + ' second(s)'
    if t.hours > 0:
        if t.minutes > 0:
            if t.seconds > 0:
                msg = h + ', ' + m + ' and ' + s
            else:
                msg = h + ' and ' + m
        elif t.seconds > 0:
            msg = h + ' and ' + s
        else:
            msg = h
    elif t.minutes > 0:
        if t.seconds > 0:
            msg = m + ' and ' + s
        else:
            msg = m
    else:
        msg = s
    return msg


assert seconds_to_duration(160) == Duration(hours=0, minutes=2, seconds=40)
assert seconds_to_duration(3760) == Duration(hours=1, minutes=2, seconds=40)
assert duration_to_string(Duration(0, 0, 0)) == '0 second(s)'
assert duration_to_string(Duration(0, 0, 1)) == '1 second(s)'
assert duration_to_string(Duration(0, 0, 10)) == '10 second(s)'
assert duration_to_string(Duration(0, 1, 20)) == '1 minute(s) and 20 second(s)'
assert duration_to_string(Duration(0, 2, 0)) == '2 minute(s)'
assert duration_to_string(Duration(1, 2, 1)) == '1 hour(s), 2 minute(s) and 1 second(s)'
assert duration_to_string(Duration(4, 0, 25)) == '4 hour(s) and 25 second(s)'
assert duration_to_string(Duration(2, 4, 0)) == '2 hour(s) and 4 minute(s)'
assert duration_to_string(Duration(3, 0, 0)) == '3 hour(s)'
