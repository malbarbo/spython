FIM = object()
def f(x: object) -> bool:
    return x is FIM
print(f(FIM), f(1))
