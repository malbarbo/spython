def nome_eh_paula(nome_completo: str) -> bool:
    '''
    Produz True se o primeiro nome de *nome_completo* é Paula, False caso contrário.

    Requer que *nome_completo* não começe e nem termine com espaços e que
    contenha pelo menos um espaço em branco.

    Exemplos
    >>> nome_eh_paula('Paula da Silva')
    True
    >>> nome_eh_paula('Paulah de Maringá')
    False
    >>> nome_eh_paula('J B')
    False
    '''
    return nome_completo[:6] == 'Paula '

def sobrenome_eh_silva(nome_completo: str) -> bool:
    '''
    Produz True se o último nome (sobrenome) de *nome_completo* é Silva, False
    caso contrário.

    Requer que *nome_completo* não começe e nem termine com espaços e que
    contenha pelo menos um espaço em branco.

    Exemplos
    >>> nome_eh_paula('Paula da Silva')
    True
    >>> nome_eh_paula('João SaSilva')
    False
    >>> nome_eh_paula('J B')
    False
    '''
    return nome_completo[-6:] == ' Silva'

# Generated from doctests.
assert nome_eh_paula('Paula da Silva') == True
assert nome_eh_paula('Paulah de Maringá') == False
assert nome_eh_paula('J B') == False
assert nome_eh_paula('Paula da Silva') == True
assert nome_eh_paula('João SaSilva') == False
assert nome_eh_paula('J B') == False
