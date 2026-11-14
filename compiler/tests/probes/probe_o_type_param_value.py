class A:
    def __init__(self) -> None:
        pass
def mk(c: type[A]) -> A:
    return c()
print(type(mk(A)).__name__)
