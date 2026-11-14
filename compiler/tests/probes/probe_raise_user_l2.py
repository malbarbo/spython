class Erro(Exception):
    pass
def f() -> None:
    raise Erro("x")
f()
