def first_name_is_paula(full_name: str) -> bool:
    '''
    Returns True if the first name in *full_name* is Paula, False otherwise.
    Requires *full_name* to not start or end with spaces and to contain at least one space.

    Examples
    >>> first_name_is_paula('Paula da Silva')
    True
    >>> first_name_is_paula('Paulah de Maringa')
    False
    >>> first_name_is_paula('J B')
    False
    '''
    return full_name[:6] == 'Paula '


def last_name_is_silva(full_name: str) -> bool:
    '''
    Returns True if the last name in *full_name* is Silva, False otherwise.
    Requires *full_name* to not start or end with spaces and to contain at least one space.

    Examples
    >>> last_name_is_silva('Paula da Silva')
    True
    >>> last_name_is_silva('Joao SaSilva')
    False
    >>> last_name_is_silva('J B')
    False
    '''
    return full_name[-6:] == ' Silva'


assert first_name_is_paula('Paula da Silva') == True
assert first_name_is_paula('Paulah de Maringa') == False
assert first_name_is_paula('J B') == False
assert last_name_is_silva('Paula da Silva') == True
assert last_name_is_silva('Joao SaSilva') == False
assert last_name_is_silva('J B') == False
