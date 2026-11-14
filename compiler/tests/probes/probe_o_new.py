class C:
    def __new__(cls) -> "C":
        return super().__new__(cls)
print(type(C()).__name__)
