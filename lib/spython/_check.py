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


def _positive(**measures: float) -> None:
    """Raises `ValueError` if one of `measures` is 0, negative, infinite, or
    NaN."""
    _finite(**measures)
    for name, value in measures.items():
        if value <= 0:
            raise ValueError(f"{name} must be positive, got {value}")


def _between(low: float, high: float, **values: float) -> None:
    """Raises `ValueError` if one of `values` is less than `low` or greater
    than `high`."""
    for name, value in values.items():
        if not low <= value <= high:
            raise ValueError(
                f"{name} must be between {low:g} and {high:g}, got {value}"
            )


def _at_least(minimum: int, **counts: int) -> None:
    """Raises `ValueError` if one of `counts` is less than `minimum`."""
    for name, value in counts.items():
        if value < minimum:
            raise ValueError(f"{name} must be at least {minimum}, got {value}")


def _angle_below(limit: float, **angles: float) -> None:
    """Raises `ValueError` if one of `angles` is not between 0 and `limit`,
    both excluded."""
    for name, value in angles.items():
        if not 0 < value < limit:
            raise ValueError(
                f"{name} must be greater than 0 and less than {limit:g}, got {value}"
            )
