class C:
    def __getattr__(self, n: str) -> int:
        return 1
print(C().x)
