# Test script that imports a student package named like the library

from spython_helpers import missing_annotations


def main() -> None:
    print(missing_annotations(10, 20))


main()
