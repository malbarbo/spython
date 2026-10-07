# Test script that imports a module with from, not a name in it

from test_package.subpackage.nested import deep_module


def main() -> None:
    print(deep_module.missing_annotations(10, 20))


main()
