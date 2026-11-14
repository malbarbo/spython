class Erro(Exception):
    pass
try:
    raise Erro("x")
except Erro as e:
    print(e)
