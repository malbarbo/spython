"""The checks that raise `ValueError` for an argument out of range. The
name of each keyword argument is the name that the message shows."""

import math as _math


def _finite(**values: float) -> None:
    """Raises `ValueError` if one of `values` is infinite or NaN."""
    for name, value in values.items():
        if not _math.isfinite(value):
            raise ValueError(f"{name} must be a finite number, got {value}")


def _non_negative(**measures: float) -> None:
    """Raises `ValueError` if one of `measures` is negative, infinite, or
    NaN."""
    _finite(**measures)
    for name, value in measures.items():
        if value < 0:
            raise ValueError(f"{name} must not be negative, got {value}")


def _between(low: float, high: float, **values: float) -> None:
    """Raises `ValueError` if one of `values` is less than `low` or greater
    than `high`."""
    for name, value in values.items():
        if not low <= value <= high:
            raise ValueError(
                f"{name} must be between {low:g} and {high:g}, got {value}"
            )
