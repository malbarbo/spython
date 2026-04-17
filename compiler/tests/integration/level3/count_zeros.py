def count_zeros(a: list[list[int]]) -> int:
    '''
    Counts the number of zeros in matrix *a*.

    Examples
    >>> count_zeros([[1, 0, 7], [0, 1, 0]])
    3
    >>> count_zeros([[1, 0], [1, 2], [0, 2]])
    2
    '''
    num_zeros = 0
    for i in range(len(a)):
        for j in range(len(a[i])):
            if a[i][j] == 0:
                num_zeros = num_zeros + 1
    return num_zeros


def count_zeros2(a: list[list[int]]) -> int:
    '''
    Counts the number of zeros in matrix *a*.

    Examples
    >>> count_zeros2([[1, 0, 7], [0, 1, 0]])
    3
    >>> count_zeros2([[1, 0], [1, 2], [0, 2]])
    2
    '''
    num_zeros = 0
    for row in a:
        for elem in row:
            if elem == 0:
                num_zeros = num_zeros + 1
    return num_zeros


assert count_zeros([[1, 0, 7], [0, 1, 0]]) == 3
assert count_zeros([[1, 0], [1, 2], [0, 2]]) == 2
assert count_zeros2([[1, 0, 7], [0, 1, 0]]) == 3
assert count_zeros2([[1, 0], [1, 2], [0, 2]]) == 2
