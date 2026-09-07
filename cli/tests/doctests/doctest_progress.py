import sys


def noisy() -> int:
    """
    >>> noisy()
    1
    """
    sys.stderr.write("during example\n")
    return 1
