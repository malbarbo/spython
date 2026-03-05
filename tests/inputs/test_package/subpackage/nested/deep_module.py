# Deep module - demonstrates three-level relative import (from ...X import Y)

from typing import List

# Three levels up: from test_package.module_b import function_b
from ...module_b import function_b, use_helper


def deep_function(items: List[str]) -> List[str]:
    """Use function from three levels up."""
    # function_b is from test_package/module_b.py (three levels up)
    return function_b(items)


def deep_helper(value: int) -> int:
    """Use helper from three levels up."""
    # use_helper is from test_package/module_b.py (three levels up)
    return use_helper(value) * 2


def missing_annotations(x, y):
    """This function has missing parameter and return annotations."""
    return x + y
