class C:
    def __reversed__(self) -> list[int]:
        return [3, 2, 1]
print(reversed(C()))
