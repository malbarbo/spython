def create_zero_matrix(m: int, n: int) -> list[list[int]]:
    '''
    Creates a zero matrix with *m* rows and *n* columns.

    Requires m > 0 and n > 0.

    Examples
    >>> create_zero_matrix(2, 3)
    [[0, 0, 0], [0, 0, 0]]
    '''
    a = []
    for i in range(m):
        row = []
        for j in range(n):
            row.append(0)
        a.append(row)
    return a


assert create_zero_matrix(2, 3) == [[0, 0, 0], [0, 0, 0]]
