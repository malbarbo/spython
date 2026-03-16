# Test file with various import styles

import os
import sys
from typing import Dict, List


def process_data(data: List[str]) -> Dict[str, int]:
    return {item: len(item) for item in data}


def main() -> None:
    print("Testing imports")


if __name__ == "__main__":
    main()
