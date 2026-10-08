def outer() -> int:
    def inner() -> int:
        """
        >>> inner()
        1
        """
        return 1

    return inner()
