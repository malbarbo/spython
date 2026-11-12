"""The class of `set`, whose copy for the type of the items is the class of a `set` of the
program.

The class follows `setobject.c` of CPython 3.13. Each slot of the table holds an item and its
`hash`, or is empty, or is the slot of a removed item. A search looks at the 9 slots after a
slot before it jumps, as CPython does, so the items come out in the order of CPython."""

from __spython_prelude__.intrinsics import (
    Hash,
    hash_eq,
    hash_of,
    hash_shift,
    hash_slot,
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
