class A:
    """doc"""
    def f(self) -> int:
        """doc"""
        ...
        return 1
print(A().f())
