# Module B in test package

from typing import List

from .module_a import helper_a


def function_b(items: List[str]) -> List[str]:
    """Convert all items to uppercase."""
    return [item.upper() for item in items]


def use_helper(value: int) -> int:
    """Use helper from module_a."""
    return helper_a(value) + 10
