"""`list.sort` and `sorted`, a port of the Timsort of CPython 3.13, in `Objects/listobject.c`.
The port makes the same comparisons in the same order, so a `__lt__` that writes or that is not
a total order, and a list with a NaN, give what CPython gives."""

# The checker of spython takes `Callable` from `typing` alone.
from typing import Callable  # noqa: UP035

from __spython_prelude__.intrinsics import (
    Ordered,
    copy_items,
    item,
    list_capacity,
    list_reserve,
    list_set_len,
    list_truncate,
    set_item,
)
from __spython_prelude__.list import copy


def sort[T: Ordered](xs: list[T], reverse: bool) -> None:
    """`xs.sort(reverse=reverse)`. The list is empty while the sort runs, so a comparison that
    reads it sees no item, and one that changes it makes the sort raise at the end."""
    items = take(xs)
    sort_items(items, reverse)
    give_back(xs, items)


def sort_key[T, K: Ordered](xs: list[T], key: Callable[[T], K], reverse: bool) -> None:
    """`xs.sort(key=key, reverse=reverse)`."""
    items = take(xs)
    sort_items_key(items, key, reverse)
    give_back(xs, items)


def sorted_of[T: Ordered](xs: list[T], reverse: bool) -> list[T]:
    items = copy(xs)
    sort_items(items, reverse)
    return items


def sorted_key[T, K: Ordered](
    xs: list[T], key: Callable[[T], K], reverse: bool
) -> list[T]:
    items = copy(xs)
    sort_items_key(items, key, reverse)
    return items


def sort_items[T: Ordered](items: list[T], reverse: bool) -> None:
    if len(items) > 1:
        empty: list[T] = []
        if reverse:
            reverse_items(items, 0, len(items))
        Sort(items, empty, False).run()
        if reverse:
            reverse_items(items, 0, len(items))


def sort_items_key[T, K: Ordered](
    items: list[T], key: Callable[[T], K], reverse: bool
) -> None:
    """Calls `key` once for each item, in order, before any comparison."""
    keys: list[K] = []
    list_reserve(keys, len(items))
    i = 0
    while i < len(items):
        keys.append(key(item(items, i)))
        i += 1
    if len(items) > 1:
        if reverse:
            reverse_items(keys, 0, len(keys))
            reverse_items(items, 0, len(items))
        Sort(keys, items, True).run()
        if reverse:
            reverse_items(items, 0, len(items))


def take[T](xs: list[T]) -> list[T]:
    """A new list with the items of `xs`, which becomes empty and without an array."""
    items = copy(xs)
    list_truncate(xs, 0)
    list_reserve(xs, 0)
    return items


def give_back[T](xs: list[T], items: list[T]) -> None:
    """Puts `items` back in `xs`, the list that `take` emptied. Anything that the sort put in
    `xs` goes away, and a list that gained an array raises."""
    changed = len(xs) != 0 or list_capacity(xs) != 0
    list_truncate(xs, 0)
    list_reserve(xs, len(items))
    copy_items(xs, 0, items, 0, len(items))
    list_set_len(xs, len(items))
    if changed:
        raise ValueError("list modified during sort")


def reverse_items[T](xs: list[T], lo: int, n: int) -> None:
    i = lo
    j = lo + n - 1
    while i < j:
        x = item(xs, i)
        set_item(xs, i, item(xs, j))
        set_item(xs, j, x)
        i += 1
        j -= 1


def minrun_of(n: int) -> int:
    """The least length of a run for a list of `n` items, which a shorter natural run reaches by
    binary insertion: `n` itself below 64, and else a value between 32 and 64 such that
    `n / minrun` is a power of 2 or a little below one."""
    r = 0
    while n >= 64:
        if n % 2 == 1:
            r = 1
        n = n // 2
    return n + r


def power_of(s1: int, n1: int, n2: int, n: int) -> int:
    """The power of the run at `s1` of length `n1`, followed by a run of length `n2`, in a list
    of `n` items: the depth of the node between them in the tree of merges of powersort."""
    result = 0
    a = 2 * s1 + n1
    b = a + n1 + n2
    while True:
        result += 1
        if a >= n:
            a -= n
            b -= n
        elif b >= n:
            break
        a = a * 2
        b = b * 2
    return result


def gallop_left[K: Ordered](key: K, a: list[K], base: int, n: int, hint: int) -> int:
    """The index `k` in `0..n` with `a[base + k - 1] < key <= a[base + k]`, which the search
    looks for from `hint`."""
    lastofs = 0
    ofs = 1
    if item(a, base + hint) < key:
        maxofs = n - hint
        while ofs < maxofs:
            if item(a, base + hint + ofs) < key:
                lastofs = ofs
                ofs = ofs * 2 + 1
            else:
                break
        ofs = min(ofs, maxofs)
        lastofs += hint
        ofs += hint
    else:
        maxofs = hint + 1
        while ofs < maxofs:
            if item(a, base + hint - ofs) < key:
                break
            lastofs = ofs
            ofs = ofs * 2 + 1
        ofs = min(ofs, maxofs)
        k = lastofs
        lastofs = hint - ofs
        ofs = hint - k
    lastofs += 1
    while lastofs < ofs:
        m = lastofs + (ofs - lastofs) // 2
        if item(a, base + m) < key:
            lastofs = m + 1
        else:
            ofs = m
    return ofs


def gallop_right[K: Ordered](key: K, a: list[K], base: int, n: int, hint: int) -> int:
    """The index `k` in `0..n` with `a[base + k - 1] <= key < a[base + k]`, which the search
    looks for from `hint`."""
    lastofs = 0
    ofs = 1
    if key < item(a, base + hint):
        maxofs = hint + 1
        while ofs < maxofs:
            if key < item(a, base + hint - ofs):
                lastofs = ofs
                ofs = ofs * 2 + 1
            else:
                break
        ofs = min(ofs, maxofs)
        k = lastofs
        lastofs = hint - ofs
        ofs = hint - k
    else:
        maxofs = n - hint
        while ofs < maxofs:
            if key < item(a, base + hint + ofs):
                break
            lastofs = ofs
            ofs = ofs * 2 + 1
        ofs = min(ofs, maxofs)
        lastofs += hint
        ofs += hint
    lastofs += 1
    while lastofs < ofs:
        m = lastofs + (ofs - lastofs) // 2
        if key < item(a, base + m):
            ofs = m
        else:
            lastofs = m + 1
    return ofs


class Sort[K: Ordered, T]:
    """The state of a sort of `keys`, which moves the items of `values` along with the keys when
    `has_values` holds."""

    keys: list[K]
    values: list[T]
    has_values: bool
    # The room for the shorter run of a merge.
    temp_keys: list[K]
    temp_values: list[T]
    min_gallop: int
    # The runs that wait for a merge, from the first one: where each one starts, its length
    # and its power.
    bases: list[int]
    lens: list[int]
    powers: list[int]

    def __init__(self, keys: list[K], values: list[T], has_values: bool) -> None:
        self.keys = keys
        self.values = values
        self.has_values = has_values
        temp_keys: list[K] = []
        temp_values: list[T] = []
        bases: list[int] = []
        lens: list[int] = []
        powers: list[int] = []
        self.temp_keys = temp_keys
        self.temp_values = temp_values
        # The wins in a row of one run after which a merge starts to gallop.
        self.min_gallop = 7
        self.bases = bases
        self.lens = lens
        self.powers = powers

    def run(self) -> None:
        n = len(self.keys)
        minrun = minrun_of(n)
        lo = 0
        remaining = n
        while remaining > 0:
            run = self.count_run(lo, remaining)
            if run < minrun:
                force = min(remaining, minrun)
                self.binary_sort(lo, force, run)
                run = force
            self.found_new_run(run)
            self.bases.append(lo)
            self.lens.append(run)
            self.powers.append(0)
            lo += run
            remaining -= run
        self.force_collapse()

    def reverse(self, lo: int, n: int) -> None:
        reverse_items(self.keys, lo, n)
        if self.has_values:
            reverse_items(self.values, lo, n)

    def binary_sort(self, lo: int, n: int, ok: int) -> None:
        """Sorts the `n` items at `lo` by binary insertion, where the first `ok` are sorted."""
        keys = self.keys
        ok = max(ok, 1)
        while ok < n:
            left = 0
            right = ok
            pivot = item(keys, lo + ok)
            while left < right:
                m = (left + right) // 2
                if pivot < item(keys, lo + m):
                    right = m
                else:
                    left = m + 1
            copy_items(keys, lo + left + 1, keys, lo + left, ok - left)
            set_item(keys, lo + left, pivot)
            if self.has_values:
                value = item(self.values, lo + ok)
                copy_items(
                    self.values, lo + left + 1, self.values, lo + left, ok - left
                )
                set_item(self.values, lo + left, value)
            ok += 1

    def count_run(self, lo: int, remaining: int) -> int:
        """The length of the run at `lo`, of at most `remaining` items, which the method makes
        ascending. A descending run turns around, with each run of equal items in it reversed
        first, so the sort stays stable."""
        keys = self.keys
        n = 1
        while n < remaining:
            if item(keys, lo + n) < item(keys, lo + n - 1):
                break
            n += 1
        if n == remaining:
            return n
        # An ascending run of more than one item ends here, unless its items are all equal.
        if n > 1:
            if item(keys, lo) < item(keys, lo + n - 1):
                return n
            self.reverse(lo, n)
        n += 1
        neq = 0
        while n < remaining:
            if item(keys, lo + n) < item(keys, lo + n - 1):
                if neq > 0:
                    neq += 1
                    self.reverse(lo + n - neq, neq)
                    neq = 0
            elif item(keys, lo + n - 1) < item(keys, lo + n):
                break
            else:
                neq += 1
            n += 1
        if neq > 0:
            neq += 1
            self.reverse(lo + n - neq, neq)
        self.reverse(lo, n)
        while n < remaining:
            if item(keys, lo + n) < item(keys, lo + n - 1):
                break
            n += 1
        return n

    def found_new_run(self, n2: int) -> None:
        """Merges the runs on the stack whose power is greater than the power of the last run,
        which a run of `n2` items follows."""
        if len(self.lens) == 0:
            return
        top = len(self.lens) - 1
        power = power_of(self.bases[top], self.lens[top], n2, len(self.keys))
        while len(self.lens) > 1 and self.powers[len(self.lens) - 2] > power:
            self.merge_at(len(self.lens) - 2)
        self.powers[len(self.lens) - 1] = power

    def force_collapse(self) -> None:
        while len(self.lens) > 1:
            n = len(self.lens) - 2
            if n > 0 and self.lens[n - 1] < self.lens[n + 1]:
                n -= 1
            self.merge_at(n)

    def merge_at(self, i: int) -> None:
        """Merges the runs `i` and `i + 1` of the stack."""
        a = self.bases[i]
        na = self.lens[i]
        b = self.bases[i + 1]
        nb = self.lens[i + 1]
        self.lens[i] = na + nb
        last = len(self.lens) - 1
        if i == last - 2:
            self.bases[i + 1] = self.bases[i + 2]
            self.lens[i + 1] = self.lens[i + 2]
            self.powers[i + 1] = self.powers[i + 2]
        self.bases.pop()
        self.lens.pop()
        self.powers.pop()
        # The items of the first run before the first item of the second one, and the items of
        # the second run after the last item of the first one, stay where they are.
        k = gallop_right(item(self.keys, b), self.keys, a, na, 0)
        a += k
        na -= k
        if na == 0:
            return
        nb = gallop_left(item(self.keys, a + na - 1), self.keys, b, nb, nb - 1)
        if nb <= 0:
            return
        if na <= nb:
            self.merge_lo(a, na, b, nb)
        else:
            self.merge_hi(a, na, b, nb)

    def save(self, src: int, n: int) -> None:
        """Copies the `n` items at `src` to the room of the merge."""
        if list_capacity(self.temp_keys) < n:
            list_reserve(self.temp_keys, n)
            if self.has_values:
                list_reserve(self.temp_values, n)
        copy_items(self.temp_keys, 0, self.keys, src, n)
        list_set_len(self.temp_keys, n)
        if self.has_values:
            copy_items(self.temp_values, 0, self.values, src, n)
            list_set_len(self.temp_values, n)

    def put(self, dest: int, saved: bool, src: int, n: int) -> None:
        """Copies `n` items to `dest`, from `src` of the room of the merge when `saved`, and of
        the list otherwise."""
        if saved:
            copy_items(self.keys, dest, self.temp_keys, src, n)
            if self.has_values:
                copy_items(self.values, dest, self.temp_values, src, n)
        else:
            copy_items(self.keys, dest, self.keys, src, n)
            if self.has_values:
                copy_items(self.values, dest, self.values, src, n)

    def merge_lo(self, a: int, na: int, b: int, nb: int) -> None:
        """Merges the run of `na` items at `a` with the run of `nb` items after it, with
        `na <= nb`, from the left. The first run goes to the room of the merge."""
        keys = self.keys
        temp = self.temp_keys
        self.save(a, na)
        dest = a
        pa = 0
        self.put(dest, False, b, 1)
        dest += 1
        b += 1
        nb -= 1
        # The last item of the first run goes last when only it remains.
        copy_b = False
        if nb > 0 and na == 1:
            copy_b = True
        elif nb > 0:
            min_gallop = self.min_gallop
            done = False
            while True:
                acount = 0
                bcount = 0
                while True:
                    if item(keys, b) < item(temp, pa):
                        self.put(dest, False, b, 1)
                        dest += 1
                        b += 1
                        bcount += 1
                        acount = 0
                        nb -= 1
                        if nb == 0:
                            done = True
                            break
                        if bcount >= min_gallop:
                            break
                    else:
                        self.put(dest, True, pa, 1)
                        dest += 1
                        pa += 1
                        acount += 1
                        bcount = 0
                        na -= 1
                        if na == 1:
                            copy_b = True
                            done = True
                            break
                        if acount >= min_gallop:
                            break
                if done:
                    break
                # One run wins so often that a search may skip many items.
                min_gallop += 1
                while True:
                    if min_gallop > 1:
                        min_gallop -= 1
                    self.min_gallop = min_gallop
                    k = gallop_right(item(keys, b), temp, pa, na, 0)
                    acount = k
                    if k > 0:
                        self.put(dest, True, pa, k)
                        dest += k
                        pa += k
                        na -= k
                        if na == 1:
                            copy_b = True
                            done = True
                            break
                        # Only a comparison that is not a total order gets here.
                        if na == 0:
                            done = True
                            break
                    self.put(dest, False, b, 1)
                    dest += 1
                    b += 1
                    nb -= 1
                    if nb == 0:
                        done = True
                        break
                    k = gallop_left(item(temp, pa), keys, b, nb, 0)
                    bcount = k
                    if k > 0:
                        self.put(dest, False, b, k)
                        dest += k
                        b += k
                        nb -= k
                        if nb == 0:
                            done = True
                            break
                    self.put(dest, True, pa, 1)
                    dest += 1
                    pa += 1
                    na -= 1
                    if na == 1:
                        copy_b = True
                        done = True
                        break
                    if acount < 7 and bcount < 7:
                        break
                if done:
                    break
                min_gallop += 1
                self.min_gallop = min_gallop
        if copy_b:
            self.put(dest, False, b, nb)
            self.put(dest + nb, True, pa, 1)
        elif na > 0:
            self.put(dest, True, pa, na)

    def merge_hi(self, a: int, na: int, b: int, nb: int) -> None:
        """Merges the run of `na` items at `a` with the run of `nb` items after it, with
        `na >= nb`, from the right. The second run goes to the room of the merge."""
        keys = self.keys
        temp = self.temp_keys
        dest = b + nb - 1
        self.save(b, nb)
        pb = nb - 1
        pa = a + na - 1
        self.put(dest, False, pa, 1)
        dest -= 1
        pa -= 1
        na -= 1
        # The first item of the second run goes first when only it remains.
        copy_a = False
        if na > 0 and nb == 1:
            copy_a = True
        elif na > 0:
            min_gallop = self.min_gallop
            done = False
            while True:
                acount = 0
                bcount = 0
                while True:
                    if item(temp, pb) < item(keys, pa):
                        self.put(dest, False, pa, 1)
                        dest -= 1
                        pa -= 1
                        acount += 1
                        bcount = 0
                        na -= 1
                        if na == 0:
                            done = True
                            break
                        if acount >= min_gallop:
                            break
                    else:
                        self.put(dest, True, pb, 1)
                        dest -= 1
                        pb -= 1
                        bcount += 1
                        acount = 0
                        nb -= 1
                        if nb == 1:
                            copy_a = True
                            done = True
                            break
                        if bcount >= min_gallop:
                            break
                if done:
                    break
                # One run wins so often that a search may skip many items.
                min_gallop += 1
                while True:
                    if min_gallop > 1:
                        min_gallop -= 1
                    self.min_gallop = min_gallop
                    k = na - gallop_right(item(temp, pb), keys, a, na, na - 1)
                    acount = k
                    if k > 0:
                        dest -= k
                        pa -= k
                        self.put(dest + 1, False, pa + 1, k)
                        na -= k
                        if na == 0:
                            done = True
                            break
                    self.put(dest, True, pb, 1)
                    dest -= 1
                    pb -= 1
                    nb -= 1
                    if nb == 1:
                        copy_a = True
                        done = True
                        break
                    k = nb - gallop_left(item(keys, pa), temp, 0, nb, nb - 1)
                    bcount = k
                    if k > 0:
                        dest -= k
                        pb -= k
                        self.put(dest + 1, True, pb + 1, k)
                        nb -= k
                        if nb == 1:
                            copy_a = True
                            done = True
                            break
                        # Only a comparison that is not a total order gets here.
                        if nb == 0:
                            done = True
                            break
                    self.put(dest, False, pa, 1)
                    dest -= 1
                    pa -= 1
                    na -= 1
                    if na == 0:
                        done = True
                        break
                    if acount < 7 and bcount < 7:
                        break
                if done:
                    break
                min_gallop += 1
                self.min_gallop = min_gallop
        if copy_a:
            self.put(dest + 1 - na, False, pa + 1 - na, na)
            dest -= na
            pa -= na
            self.put(dest, True, pb, 1)
        elif nb > 0:
            self.put(dest - (nb - 1), True, 0, nb)
