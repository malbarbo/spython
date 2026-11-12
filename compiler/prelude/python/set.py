"""The class of `set`, whose copy for the type of the items is the class of a `set` of the
program.

The class follows `setobject.c` of CPython 3.13. Each slot of the table holds an item and its
`hash`, or is empty, or is the slot of a removed item. A search looks at the 9 slots after a
slot before it jumps, as CPython does, so the items come out in the order of CPython."""

from __spython_prelude__.intrinsics import (
    Hash,
    during_repr,
    exact,
    hash_eq,
    hash_of,
    hash_shift,
    hash_slot,
    in_repr,
    item,
    list_reserve,
    list_set_len,
    same,
    set_item,
)


class Set[T]:
    # The item of each slot. Only a slot in use holds one.
    keys: list[T]
    hashes: list[Hash]
    # 0 for an empty slot, 1 for a slot in use and 2 for the slot of a removed item.
    states: list[int]
    # How many slots are not empty.
    fill: int
    used: int
    mask: int

    def __init__(self) -> None:
        keys: list[T] = []
        hashes: list[Hash] = []
        states: list[int] = []
        self.keys = keys
        self.hashes = hashes
        self.states = states
        self.fill = 0
        self.used = 0
        self.mask = 0
        self.new_table(8)

    def __len__(self) -> int:
        return self.used

    def __contains__(self, key: T) -> bool:
        return self.find(key, hash_of(key)) >= 0

    def add(self, key: T) -> None:
        self.add_entry(key, hash_of(key))

    def remove(self, key: T) -> None:
        if not self.discard_entry(key, hash_of(key)):
            raise KeyError(key)

    def discard(self, key: T) -> None:
        self.discard_entry(key, hash_of(key))

    def clear(self) -> None:
        self.new_table(8)
        self.fill = 0
        self.used = 0

    def done(self, items: list[T]) -> Set[T]:
        """Puts `items` in the `set`, in order, and gives the `set`, as `BUILD_SET` does."""
        for x in items:
            self.add(x)
        return self

    def new_table(self, size: int) -> None:
        """Gives the `set` an empty table of `size` slots, and keeps `fill` and `used`."""
        keys: list[T] = []
        list_reserve(keys, size)
        list_set_len(keys, size)
        hashes: list[Hash] = []
        list_reserve(hashes, size)
        list_set_len(hashes, size)
        states: list[int] = []
        list_reserve(states, size)
        for i in range(size):
            set_item(states, i, 0)
        list_set_len(states, size)
        self.keys = keys
        self.hashes = hashes
        self.states = states
        self.mask = size - 1

    def put(self, i: int, key: T, h: Hash) -> None:
        set_item(self.keys, i, key)
        set_item(self.hashes, i, h)
        set_item(self.states, i, 1)

    def changed(self, keys: list[T], i: int, key: T) -> bool:
        """Returns `True` if the comparison with `key`, the item of slot `i` of the table `keys`,
        changed the table or the slot, `False` otherwise. An item without a reference has no
        `__eq__` that changes the `set`."""
        if not same(self.keys, keys) or item(self.states, i) != 1:
            return True
        return same(key, key) and not same(item(keys, i), key)

    def find(self, key: T, h: Hash) -> int:
        """The slot of `key`, whose `hash` is `h`, or -1 when the `set` has no such item. The item
        of the slot is on the left of `==`. A comparison that changes the table or the slot starts
        the search again, as `set_lookkey` does."""
        keys = self.keys
        states = self.states
        mask = self.mask
        i = hash_slot(h, mask)
        perturb = h
        while True:
            j = i
            last = i + 9 if i + 9 <= mask else i
            while j <= last:
                state = item(states, j)
                if state == 0:
                    return -1
                if state == 1 and hash_eq(item(self.hashes, j), h):
                    k = item(keys, j)
                    if same(k, key):
                        return j
                    equal = k == key
                    if self.changed(keys, j, k):
                        return self.find(key, h)
                    if equal:
                        return j
                j += 1
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % (mask + 1)

    def add_entry(self, key: T, h: Hash) -> None:
        """Puts `key`, whose `hash` is `h`, in the `set`, as `set_add_entry` does. The item goes
        to the last slot of a removed item that the search passes, or else to the empty slot
        that ends the search. A `set` that keeps an item equal to `key` keeps its own."""
        keys = self.keys
        states = self.states
        mask = self.mask
        i = hash_slot(h, mask)
        free = -1
        perturb = h
        while True:
            j = i
            last = i + 9 if i + 9 <= mask else i
            while j <= last:
                state = item(states, j)
                if state == 0:
                    if free < 0:
                        self.fill += 1
                        self.used += 1
                        self.put(j, key, h)
                        if self.fill * 5 >= mask * 3:
                            self.resize(self.used * 2 if self.used > 50000 else self.used * 4)
                        return
                    if item(states, free) != 2:
                        self.add_entry(key, h)
                        return
                    self.used += 1
                    self.put(free, key, h)
                    return
                if state == 1:
                    if hash_eq(item(self.hashes, j), h):
                        k = item(keys, j)
                        if same(k, key):
                            return
                        equal = k == key
                        if equal:
                            return
                        if self.changed(keys, j, k):
                            self.add_entry(key, h)
                            return
                else:
                    free = j
                j += 1
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % (mask + 1)

    def insert_clean(self, key: T, h: Hash) -> None:
        """Puts `key`, whose `hash` is `h`, in the first empty slot of its search, in a table
        without removed items or an item equal to `key`."""
        states = self.states
        mask = self.mask
        i = hash_slot(h, mask)
        perturb = h
        while True:
            j = i
            last = i + 9 if i + 9 <= mask else i
            while j <= last:
                if item(states, j) == 0:
                    self.put(j, key, h)
                    return
                j += 1
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % (mask + 1)

    def resize(self, minused: int) -> None:
        """Moves the items to a new table, whose size is the first power of 2 from 8 above
        `minused`, as `set_table_resize` does. A table of 8 slots without removed items stays."""
        size = 8
        while size <= minused:
            size *= 2
        if size == 8 and self.mask == 7 and self.fill == self.used:
            return
        keys = self.keys
        hashes = self.hashes
        states = self.states
        n = self.mask + 1
        self.new_table(size)
        self.fill = self.used
        for i in range(n):
            if item(states, i) == 1:
                self.insert_clean(item(keys, i), item(hashes, i))

    def discard_entry(self, key: T, h: Hash) -> bool:
        """Removes `key`, whose `hash` is `h`. Returns `True` if the `set` had `key`, `False`
        otherwise."""
        i = self.find(key, h)
        if i < 0:
            return False
        set_item(self.states, i, 2)
        self.used -= 1
        return True

    def merge(self, other: Set[T]) -> None:
        """Puts the items of `other` in the `set`, as `set_merge` does. An empty `set` takes the
        slots of `other` as they are when the two tables have the same size, and puts the items
        in the order of the slots of `other` otherwise."""
        if same(self, other) or other.used == 0:
            return
        if (self.fill + other.used) * 5 >= self.mask * 3:
            self.resize((self.used + other.used) * 2)
        if self.fill == 0 and self.mask == other.mask and other.fill == other.used:
            for i in range(other.mask + 1):
                if item(other.states, i) == 1:
                    self.put(i, item(other.keys, i), item(other.hashes, i))
            self.fill = other.fill
            self.used = other.used
            return
        if self.fill == 0:
            self.fill = other.used
            self.used = other.used
            for i in range(other.mask + 1):
                if item(other.states, i) == 1:
                    self.insert_clean(item(other.keys, i), item(other.hashes, i))
            return
        i = 0
        while i <= other.mask:
            if item(other.states, i) == 1:
                self.add_entry(item(other.keys, i), item(other.hashes, i))
            i += 1

    def copy(self) -> Set[T]:
        new: Set[T] = Set()
        new.merge(self)
        return new

    def __repr__(self) -> str:
        """The `repr` of the `set`, which is also its `str`. A `set` inside its own `repr` is
        `set(...)`, as in CPython."""
        if in_repr(self):
            return "set(...)"
        if self.used == 0:
            return "set()"
        return during_repr(self, self.items_repr())

    def items_repr(self) -> str:
        """The `repr` of a `set` with items. The `repr` of each item comes after the list of
        the items, as in `set_repr`."""
        items: list[T] = []
        for i in range(self.mask + 1):
            if item(self.states, i) == 1:
                items.append(item(self.keys, i))
        parts: list[str] = []
        for x in items:
            parts.append(repr(x))
        return "{" + ", ".join(parts) + "}"

    def iter(self) -> SetIter[T]:
        return SetIter(self)

    def __eq__(self, other: object) -> bool:
        """Returns `True` if `other` is a `set` with the same items, `False` otherwise."""
        o = exact(self, other)
        if o is None:
            return False
        if self.used != o.used:
            return False
        return self.issubset(o)

    def __le__(self, other: Set[T]) -> bool:
        return self.issubset(other)

    def __lt__(self, other: Set[T]) -> bool:
        if self.used >= other.used:
            return False
        return self.issubset(other)

    def __ge__(self, other: Set[T]) -> bool:
        return other.issubset(self)

    def __gt__(self, other: Set[T]) -> bool:
        if self.used <= other.used:
            return False
        return other.issubset(self)

    def issubset(self, other: Set[T]) -> bool:
        """Returns `True` if `other` has each item of the `set`, `False` otherwise. The item of
        `other` is on the left of `==`."""
        if self.used > other.used:
            return False
        i = 0
        while i <= self.mask:
            if item(self.states, i) == 1:
                if other.find(item(self.keys, i), item(self.hashes, i)) < 0:
                    return False
            i += 1
        return True

    def __or__(self, other: Set[T]) -> Set[T]:
        result = self.copy()
        if not same(self, other):
            result.merge(other)
        return result

    def __ior__(self, other: Set[T]) -> Set[T]:
        self.merge(other)
        return self

    def __and__(self, other: Set[T]) -> Set[T]:
        """The items of both `set`s, as `set_intersection` gives them. The search goes over the
        smaller `set` and looks for each item in the other one."""
        if same(self, other):
            return self.copy()
        result: Set[T] = Set()
        big = self
        small = other
        if other.used > self.used:
            big = other
            small = self
        i = 0
        while i <= small.mask:
            if item(small.states, i) == 1:
                key = item(small.keys, i)
                h = item(small.hashes, i)
                if big.find(key, h) >= 0:
                    result.add_entry(key, h)
            i += 1
        return result

    def __iand__(self, other: Set[T]) -> Set[T]:
        """Keeps the items that `other` has. The `set` takes the table of the intersection, as
        `set_intersection_update` does."""
        result = self & other
        self.keys = result.keys
        self.hashes = result.hashes
        self.states = result.states
        self.fill = result.fill
        self.used = result.used
        self.mask = result.mask
        return self

    def __sub__(self, other: Set[T]) -> Set[T]:
        """The items of the `set` that `other` does not have, as `set_difference` gives them. A
        copy of a `set` with more than 4 times the items of `other` loses the items of `other`."""
        if self.used // 4 > other.used:
            result = self.copy()
            result.remove_all(other)
            return result
        rest: Set[T] = Set()
        i = 0
        while i <= self.mask:
            if item(self.states, i) == 1:
                key = item(self.keys, i)
                h = item(self.hashes, i)
                if other.find(key, h) < 0:
                    rest.add_entry(key, h)
            i += 1
        return rest

    def __isub__(self, other: Set[T]) -> Set[T]:
        self.remove_all(other)
        return self

    def remove_all(self, other: Set[T]) -> None:
        """Removes the items of `other`, as `set_difference_update_internal` does. When `other`
        has more than 8 times the items of the `set`, the items to remove are the ones of both."""
        if same(self, other):
            self.clear()
            return
        if other.used // 8 > self.used:
            other = self & other
        i = 0
        while i <= other.mask:
            if item(other.states, i) == 1:
                self.discard_entry(item(other.keys, i), item(other.hashes, i))
            i += 1
        if self.fill - self.used > self.mask // 4:
            self.resize(self.used * 2 if self.used > 50000 else self.used * 4)

    def __xor__(self, other: Set[T]) -> Set[T]:
        """The items of one `set` alone, as `set_symmetric_difference` gives them. The result
        takes the items of `other`, and then each item of the `set` goes in or out of it."""
        result: Set[T] = Set()
        result.merge(other)
        result.toggle_all(self)
        return result

    def __ixor__(self, other: Set[T]) -> Set[T]:
        if same(self, other):
            self.clear()
        else:
            self.toggle_all(other)
        return self

    def toggle_all(self, other: Set[T]) -> None:
        """Removes each item of `other` that the `set` has, and puts in the others, as
        `set_symmetric_difference_update_set` does."""
        i = 0
        while i <= other.mask:
            if item(other.states, i) == 1:
                key = item(other.keys, i)
                h = item(other.hashes, i)
                if not self.discard_entry(key, h):
                    self.add_entry(key, h)
            i += 1

class SetIter[T]:
    """A `for` over a `set`, which follows `setiter_iternext` of CPython 3.13."""

    s: Set[T]
    # The first slot that the loop has yet to see.
    pos: int
    # The size of the `set` when the loop started.
    used: int
    # The slot of the last item that `next` found.
    index: int

    def __init__(self, s: Set[T]) -> None:
        self.s = s
        self.pos = 0
        self.used = s.used
        self.index = -1

    def next(self) -> bool:
        """Finds the next item in the `set`, and returns `False` past the last one. A change of
        the size of the `set` raises `RuntimeError`."""
        s = self.s
        if self.used != s.used:
            raise RuntimeError("Set changed size during iteration")
        i = self.pos
        mask = s.mask
        while i <= mask and item(s.states, i) != 1:
            i += 1
        self.pos = i + 1
        if i > mask:
            return False
        self.index = i
        return True

    def key(self) -> T:
        return item(self.s.keys, self.index)
