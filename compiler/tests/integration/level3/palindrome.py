def palindrome(lst: list[int]) -> bool:
    '''
    Returns True if *lst* is a palindrome, that is, its elements read
    the same from left to right and from right to left.

    Examples
    >>> palindrome([])
    True
    >>> palindrome([4])
    True
    >>> palindrome([1, 1])
    True
    >>> palindrome([1, 2])
    False
    >>> palindrome([1, 2, 1])
    True
    >>> palindrome([1, 5, 5, 1])
    True
    >>> palindrome([1, 5, 1, 5])
    False
    '''
    is_palindrome = True
    i = 0
    j = len(lst) - 1
    while i < j and is_palindrome:
        if lst[i] != lst[j]:
            is_palindrome = False
        i = i + 1
        j = j - 1
    return is_palindrome


assert palindrome([]) == True
assert palindrome([4]) == True
assert palindrome([1, 1]) == True
assert palindrome([1, 2]) == False
assert palindrome([1, 2, 1]) == True
assert palindrome([1, 5, 5, 1]) == True
assert palindrome([1, 5, 1, 5]) == False
