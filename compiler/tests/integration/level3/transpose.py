def transpose(a: list[list[int]]) -> list[list[int]]:
    '''
    Creates the transpose of matrix *a*.

    Requires *a* to be a regular matrix.

    Examples
    >>> transpose([[4, 5, 1], [7, 8, 9]])
    [[4, 7], [5, 8], [1, 9]]
    >>> transpose([[4, 1], [7, 8], [2, 6], [5, 3]])
    [[4, 7, 2, 5], [1, 8, 6, 3]]
    '''
    t = []
    for j in range(len(a[0])):
        col = []
        for i in range(len(a)):
            col.append(a[i][j])
        t.append(col)
    return t


assert transpose([[4, 5, 1], [7, 8, 9]]) == [[4, 7], [5, 8], [1, 9]]
assert transpose([[4, 1], [7, 8], [2, 6], [5, 3]]) == [[4, 7, 2, 5], [1, 8, 6, 3]]
