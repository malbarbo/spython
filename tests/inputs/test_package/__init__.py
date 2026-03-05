# Test package for relative imports

from .module_a import function_a
from .module_b import function_b

__all__ = ["function_a", "function_b"]


def package_function() -> str:
    return "Hello from package"
