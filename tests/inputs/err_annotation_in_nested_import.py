# Test script for nested relative imports

from test_package.subpackage.nested import nested_function
from test_package.subpackage.nested.deep_module import (
    deep_function,
    deep_helper,
    missing_annotations,
)


def main() -> None:
    # Test nested_function (uses imports from parent and grandparent)
    result = nested_function(10)
    print(f"nested_function(10) = {result}")

    # Test deep_function (uses three-level relative import: from ...module_b)
    words = ["hello", "world", "python"]
    uppercase = deep_function(words)
    print(f"deep_function({words}) = {uppercase}")

    # Test deep_helper (also uses three-level relative import)
    value = deep_helper(5)
    print(f"deep_helper(5) = {value}")

    # Test missing_annotations (should trigger annotation errors)
    result_missing = missing_annotations(10, 20)
    print(f"missing_annotations(10, 20) = {result_missing}")


if __name__ == "__main__":
    main()
