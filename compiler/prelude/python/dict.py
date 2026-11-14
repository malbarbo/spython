"""The class of `dict`, whose copy for the types of the keys and of the values is the class of a
`dict` of the program.

The class follows the compact `dict` of CPython 3.13. Each entry has a `hash`, a key and a value,
in the order of insertion. A table of indices gives the entry of each slot, -1 for an empty slot
and -2 for the slot of a removed entry. A removed entry keeps its place until the table grows, so
the growth of the table happens where it happens in CPython. The arrays of the entries have room
for the entries that the table takes, two thirds of its slots, as in CPython."""

from __spython_prelude__.arrays import copied, resized
from __spython_prelude__.intrinsics import (
    Array,
    Hash,
    array_get,
    array_len,
    array_new,
    array_set,
    during_repr,
    exact,
    hash_eq,
    hash_of,
    hash_shift,
    hash_slot,
    in_repr,
    same,
)


class Dict[K, V]:
    # The slots of the table. An empty array stands for the table of an empty `dict` of CPython,
    # which takes no entry.
    indices: Array[int]
    hashes: Array[Hash]
    keys: Array[K]
    values: Array[V]
    # Whether each entry is in the `dict`.
    live: Array[bool]
    # How many entries the arrays hold, with the removed ones.
    entries: int
    used: int
    # How many entries the table takes before it grows.
    usable: int
    # Whether the table is one that CPython keeps for keys of `str` alone. A key of another type
    # makes CPython move the entries to a new table.
    unicode: bool

    def __init__(self) -> None:
        indices: Array[int] = array_new(0)
        hashes: Array[Hash] = array_new(0)
        keys: Array[K] = array_new(0)
        values: Array[V] = array_new(0)
        live: Array[bool] = array_new(0)
        self.indices = indices
        self.hashes = hashes
        self.keys = keys
        self.values = values
        self.live = live
        self.entries = 0
        self.used = 0
        self.usable = 0
        self.unicode = True

    def __len__(self) -> int:
        return self.used

    def __getitem__(self, key: K) -> V:
        ix = self.find(key, hash_of(key))
        if ix < 0:
            raise KeyError(key)
        return array_get(self.values, ix)

    def __setitem__(self, key: K, value: V) -> None:
        self.insert(key, hash_of(key), value)

    def __eq__(self, other: object) -> bool:
        """Returns `True` if `other` is a `dict` with the same keys, each with an equal value,
        `False` otherwise. Each value of this `dict` is on the left of `==`, as in `dict_equal`."""
        o = exact(self, other)
        if o is None:
            return False
        if self.used != o.used:
            return False
        i = 0
        while i < self.entries:
            if array_get(self.live, i):
                value = array_get(self.values, i)
                ix = o.find(array_get(self.keys, i), array_get(self.hashes, i))
                if ix < 0:
                    return False
                found = array_get(o.values, ix)
                if not (same(value, found) or value == found):
                    return False
            i += 1
        return True

    def __contains__(self, key: K) -> bool:
        return self.find(key, hash_of(key)) >= 0

    def get(self, key: K) -> V | None:
        ix = self.find(key, hash_of(key))
        if ix < 0:
            return None
        return array_get(self.values, ix)

    def get_or[D](self, key: K, default: D) -> V | D:
        ix = self.find(key, hash_of(key))
        if ix < 0:
            return default
        return array_get(self.values, ix)

    def get_or_none(self, key: K) -> V | None:
        return self.get(key)

    def pop(self, key: K) -> V:
        ix = self.remove(key)
        if ix < 0:
            raise KeyError(key)
        return array_get(self.values, ix)

    def pop_or[D](self, key: K, default: D) -> V | D:
        ix = self.remove(key)
        if ix < 0:
            return default
        return array_get(self.values, ix)

    def pop_or_none(self, key: K) -> V | None:
        ix = self.remove(key)
        if ix < 0:
            return None
        return array_get(self.values, ix)

    def copy(self) -> Dict[K, V]:
        """A new `dict` with the items of this one. A table with few removed entries goes to
        the copy as it is, and the copy of another one has a new table, as in CPython."""
        new: Dict[K, V] = Dict()
        if self.used == 0:
            return new
        if self.used >= self.entries * 2 // 3:
            new.indices = copied(self.indices)
            new.hashes = copied(self.hashes)
            new.keys = copied(self.keys)
            new.values = copied(self.values)
            new.live = copied(self.live)
            new.entries = self.entries
            new.used = self.used
            new.usable = self.usable
            new.unicode = self.unicode
            return new
        n = self.used
        if n > 5:
            new.resize((n * 3 + 1) // 2, self.unicode)
        i = 0
        while i < self.entries:
            if array_get(self.live, i):
                new.insert(
                    array_get(self.keys, i),
                    array_get(self.hashes, i),
                    array_get(self.values, i),
                )
                if n != self.used:
                    raise RuntimeError("dict mutated during update")
            i += 1
        return new

    def __repr__(self) -> str:
        """The `repr` of the `dict`, which is also its `str`. A `dict` inside its own `repr` is
        `{...}`, as in CPython."""
        if in_repr(self):
            return "{...}"
        if self.used == 0:
            return "{}"
        return during_repr(self, self.items_repr())

    def items_repr(self) -> str:
        """The `repr` of a `dict` with items. The loop reads the entries again after each
        `repr`, which may change the `dict`, as `dict_repr` does."""
        parts: list[str] = []
        i = 0
        while i < self.entries:
            if array_get(self.live, i):
                key = array_get(self.keys, i)
                value = array_get(self.values, i)
                parts.append(repr(key) + ": " + repr(value))
            i += 1
        return "{" + ", ".join(parts) + "}"

    def clear(self) -> None:
        self.new_table(0, True)

    def iter(self) -> DictIter[K, V]:
        return DictIter(self)

    def pend(self, key: K, value: V) -> Dict[K, V]:
        """Keeps an item of a literal of `dict`, which [`done`] puts in the `dict` once each key
        and each value of the literal has its value, as `BUILD_MAP` of CPython does."""
        n = self.entries
        if n == array_len(self.keys):
            self.keys = resized(self.keys, max(8, 2 * n))
            self.values = resized(self.values, max(8, 2 * n))
        array_set(self.keys, n, key)
        array_set(self.values, n, value)
        self.entries = n + 1
        return self

    def done(self) -> Dict[K, V]:
        """Puts in the `dict` the items that `pend` kept, in a table that CPython sizes for them,
        and gives the `dict`."""
        keys = self.keys
        values = self.values
        n = self.entries
        self.clear()
        if n > 5:
            unicode = True
            for i in range(n):
                if not isinstance(array_get(keys, i), str):
                    unicode = False
                    break
            self.resize(min((n * 3 + 1) // 2, 131072), unicode)
        for i in range(n):
            key = array_get(keys, i)
            self.insert(key, hash_of(key), array_get(values, i))
        return self

    def find(self, key: K, h: Hash) -> int:
        """The index of the entry of `key`, whose `hash` is `h`, or -1 when the `dict` has no such
        key. The key of the entry is on the left of `==`. A comparison that changes the table or
        removes the entry starts the search again, as in CPython."""
        indices = self.indices
        size = array_len(indices)
        if size == 0:
            return -1
        mask = size - 1
        i = hash_slot(h, mask)
        perturb = h
        while True:
            ix = array_get(indices, i)
            if ix >= 0:
                k = array_get(self.keys, ix)
                if same(k, key):
                    return ix
                if hash_eq(array_get(self.hashes, ix), h):
                    equal = k == key
                    if not same(self.indices, indices) or not array_get(self.live, ix):
                        return self.find(key, h)
                    if equal:
                        return ix
            elif ix == -1:
                return -1
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) & mask

    def remove(self, key: K) -> int:
        """Removes the entry of `key` and gives its index, or -1 when the `dict` has no such key.
        The value of the entry stays in its place. An empty `dict` takes no `hash` of the key, as
        in CPython."""
        if self.used == 0:
            return -1
        h = hash_of(key)
        ix = self.find(key, h)
        if ix >= 0:
            array_set(self.indices, self.slot_of(h, ix), -2)
            array_set(self.live, ix, False)
            self.used -= 1
        return ix

    def slot_of(self, h: Hash, ix: int) -> int:
        """The slot of the entry at `ix`, whose `hash` is `h`."""
        indices = self.indices
        mask = array_len(indices) - 1
        i = hash_slot(h, mask)
        perturb = h
        while array_get(indices, i) != ix:
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) & mask
        return i

    def empty_slot(self, h: Hash) -> int:
        """The first slot for `h` that holds no entry."""
        indices = self.indices
        mask = array_len(indices) - 1
        i = hash_slot(h, mask)
        perturb = h
        while array_get(indices, i) >= 0:
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) & mask
        return i

    def insert(self, key: K, h: Hash, value: V) -> None:
        """Puts `value` under `key`, whose `hash` is `h`. A key that the `dict` has keeps its first
        object."""
        if array_len(self.indices) == 0:
            self.new_table(8, isinstance(key, str))
        else:
            ix = self.find(key, h)
            if ix >= 0:
                array_set(self.values, ix, value)
                return
            if self.unicode and not isinstance(key, str):
                self.resize(self.used * 3, False)
            if self.usable <= 0:
                self.resize(self.used * 3, True)
        n = self.entries
        array_set(self.indices, self.empty_slot(h), n)
        array_set(self.hashes, n, h)
        array_set(self.keys, n, key)
        array_set(self.values, n, value)
        array_set(self.live, n, True)
        self.entries = n + 1
        self.usable -= 1
        self.used += 1

    def new_table(self, size: int, unicode: bool) -> None:
        """Gives the `dict` an empty table of `size` slots, without entries, and arrays with room
        for the entries that the table takes."""
        indices: Array[int] = array_new(size)
        for i in range(size):
            array_set(indices, i, -1)
        room = size * 2 // 3
        hashes: Array[Hash] = array_new(room)
        keys: Array[K] = array_new(room)
        values: Array[V] = array_new(room)
        live: Array[bool] = array_new(room)
        self.indices = indices
        self.hashes = hashes
        self.keys = keys
        self.values = values
        self.live = live
        self.entries = 0
        self.used = 0
        self.usable = room
        self.unicode = unicode

    def resize(self, minsize: int, unicode: bool) -> None:
        """Moves the entries in the `dict` to a new table, whose size is the first power of 2 from
        8 that is not below `minsize`, as `dictresize` does."""
        size = 8
        while size < minsize:
            size *= 2
        hashes = self.hashes
        keys = self.keys
        values = self.values
        live = self.live
        n = self.entries
        used = self.used
        self.new_table(size, unicode and self.unicode)
        for i in range(n):
            if array_get(live, i):
                h = array_get(hashes, i)
                j = self.entries
                array_set(self.indices, self.empty_slot(h), j)
                array_set(self.hashes, j, h)
                array_set(self.keys, j, array_get(keys, i))
                array_set(self.values, j, array_get(values, i))
                array_set(self.live, j, True)
                self.entries = j + 1
        self.used = used
        self.usable -= self.entries


class DictIter[K, V]:
    """A `for` over a `dict`, which follows `dictiter_iternextkey` of CPython 3.13."""

    d: Dict[K, V]
    # The first entry that the loop has yet to see.
    pos: int
    # The size of the `dict` when the loop started.
    used: int
    # How many entries the loop has yet to see.
    left: int
    # The entry of the last item that `next` found.
    index: int

    def __init__(self, d: Dict[K, V]) -> None:
        self.d = d
        self.pos = 0
        self.used = d.used
        self.left = d.used
        self.index = -1

    def next(self) -> bool:
        """Finds the next entry in the `dict`, and returns `False` past the last one. A change of
        the size of the `dict`, or one more entry than the loop expects, raises `RuntimeError`."""
        d = self.d
        if self.used != d.used:
            raise RuntimeError("dictionary changed size during iteration")
        i = self.pos
        n = d.entries
        while i < n and not array_get(d.live, i):
            i += 1
        if i >= n:
            return False
        if self.left == 0:
            raise RuntimeError("dictionary keys changed during iteration")
        self.pos = i + 1
        self.left -= 1
        self.index = i
        return True

    def key(self) -> K:
        return array_get(self.d.keys, self.index)

    def value(self) -> V:
        return array_get(self.d.values, self.index)
