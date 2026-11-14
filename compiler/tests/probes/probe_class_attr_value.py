class C:
    total: int = 0
    def __init__(self) -> None:
        C.total += 1
C()
print(C.total)
