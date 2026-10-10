"""The checks that raise `ValueError` for an argument out of range. The
name of each keyword argument is the name that the message shows."""


def _non_negative(**measures: float) -> None:
    """Raises `ValueError` if one of `measures` is negative."""
    for name, value in measures.items():
        if value < 0:
            raise ValueError(f"{name} must not be negative, got {value}")


def _positive(**measures: float) -> None:
    """Raises `ValueError` if one of `measures` is 0 or negative."""
    for name, value in measures.items():
        if value <= 0:
            raise ValueError(f"{name} must be positive, got {value}")


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
