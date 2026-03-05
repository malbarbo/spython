# Test script that uses the test_package

from test_package import function_a, function_b, package_function
from test_package.module_b import use_helper


def main() -> None:
    # Test function_a
    numbers = [1, 2, 3, 4, 5]
    total = function_a(numbers)
    print(f"Sum of {numbers} = {total}")

    # Test function_b
    words = ["hello", "world"]
    uppercase = function_b(words)
    print(f"Uppercase: {uppercase}")

    # Test package function
    message = package_function()
    print(message)

    # Test relative import usage
    result = use_helper(5)
    print(f"use_helper(5) = {result}")


if __name__ == "__main__":
    main()
