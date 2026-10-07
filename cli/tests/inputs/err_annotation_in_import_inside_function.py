# Test script that imports a module inside a function


def main() -> None:
    from test_package.subpackage.nested.deep_module import missing_annotations

    print(missing_annotations(10, 20))


main()
