# Test to verify annotation checking happens before type checking


def no_annotations(x: int, y: int) -> int:
    return x + y


def with_type_error(x: int, y: int) -> int:
    return "string"  # Type error: returning str instead of int
