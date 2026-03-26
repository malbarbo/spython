# Module A in test package

from typing import List


def function_a(items: List[int]) -> int:
    """Sum all items in the list."""
    return sum(items)


def helper_a(value: int) -> int:
    """Double the value."""
    return value * 2
