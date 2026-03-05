# Direct import test for deep_module

from test_package.subpackage.nested.deep_module import missing_annotations


def main() -> None:
    result = missing_annotations(5, 10)
    print(f"Result: {result}")


if __name__ == "__main__":
    main()
