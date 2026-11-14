class MeuErro(Exception):
    pass

def f(x: int) -> int:
    if x < 0:
        raise MeuErro("negativo")
    return x

try:
    f(-1)
except MeuErro as e:
    print("meu", e)
