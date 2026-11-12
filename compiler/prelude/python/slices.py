"""The bounds of a slice of a `str` or of a list, as CPython adjusts them."""


def slice_start(start: int | None, n: int) -> int:
    """The first index of a slice of a sequence of length `n`, as CPython adjusts it."""
    if start is None:
        return 0
    if start < 0:
        return max(start + n, 0)
    return start


def slice_end(end: int | None, n: int) -> int:
    """The index after the last of a slice of a sequence of length `n`, as CPython adjusts it."""
    if end is None or end > n:
        return n
    if end < 0:
        return max(end + n, 0)
    return end


def step_bound(bound: int | None, n: int, negative: bool, missing: int) -> int:
    """A bound of a slice with a step of the sign that `negative` says, in a sequence of length
    `n`."""
    if bound is None:
        return missing
    i = bound
    if i < 0:
        i += n
        if i < 0:
            if negative:
                return -1
            return 0
    elif i >= n:
        if negative:
            return n - 1
        return n
    return i


def step_of(step: int | None) -> int:
    """The step of a slice, 1 when it is missing."""
    k = 1
    if step is not None:
        k = step
    if k == 0:
        raise ValueError("slice step cannot be zero")
    return k


def step_start(start: int | None, n: int, k: int) -> int:
    """The first index of a slice with the step `k` of a sequence of length `n`."""
    return step_bound(start, n, k < 0, n - 1 if k < 0 else 0)


def step_count(first: int, stop: int | None, n: int, k: int) -> int:
    """The number of items of a slice with the step `k` of a sequence of length `n`, from the
    index `first`."""
    if k < 0:
        last = step_bound(stop, n, True, -1)
        if last < first:
            return (last - first + 1) // k + 1
    else:
        last = step_bound(stop, n, False, n)
        if first < last:
            return (last - first - 1) // k + 1
    return 0
