# Nested package init - demonstrates multi-level relative imports

from typing import Dict

# Import from grandparent package (two levels up)
from ...module_a import function_a, helper_a

# Import from parent package (one level up)
from .. import subpackage_function


def nested_function(value: int) -> Dict[str, int]:
    """Use functions from parent and grandparent packages."""
    return {
        "doubled": helper_a(value),
        "sum": function_a([1, 2, 3]),
        "count": subpackage_function([1, 2, 3, 4]),
    }
