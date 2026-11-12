"""The class of `dict`, whose copy for the types of the keys and of the values is the class of a
`dict` of the program.

The class follows the compact `dict` of CPython 3.13. Each entry has a `hash`, a key and a value,
in the order of insertion. A table of indices gives the entry of each slot, -1 for an empty slot
and -2 for the slot of a removed entry. A removed entry keeps its place until the table grows, so
the growth of the table happens where it happens in CPython."""

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


class Dict[K, V]:
    # The slots of the table. An empty list stands for the table of an empty `dict` of CPython,
    # which takes no entry.
    indices: list[int]
    hashes: list[Hash]
    keys: list[K]
    values: list[V]
    # Whether each entry is in the `dict`.
    live: list[bool]
    used: int
    # How many entries the table takes before it grows.
    usable: int
    # Whether the table is one that CPython keeps for keys of `str` alone. A key of another type
    # makes CPython move the entries to a new table.
    unicode: bool

    def __init__(self) -> None:
        indices: list[int] = []
        hashes: list[Hash] = []
        keys: list[K] = []
        values: list[V] = []
        live: list[bool] = []
        self.indices = indices
        self.hashes = hashes
        self.keys = keys
        self.values = values
        self.live = live
        self.used = 0
        self.usable = 0
        self.unicode = True

    def __len__(self) -> int:
        return self.used

    def __getitem__(self, key: K) -> V:
        ix = self.find(key, hash_of(key))
        if ix < 0:
            raise KeyError(key)
        return item(self.values, ix)

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
        while i < len(self.keys):
            if item(self.live, i):
                value = item(self.values, i)
                ix = o.find(item(self.keys, i), item(self.hashes, i))
                if ix < 0:
                    return False
                found = item(o.values, ix)
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
        return item(self.values, ix)

    def get_or[D](self, key: K, default: D) -> V | D:
        ix = self.find(key, hash_of(key))
        if ix < 0:
            return default
        return item(self.values, ix)

    def get_or_none(self, key: K) -> V | None:
        return self.get(key)

    def pop(self, key: K) -> V:
        ix = self.remove(key)
        if ix < 0:
            raise KeyError(key)
        return item(self.values, ix)

    def pop_or[D](self, key: K, default: D) -> V | D:
        ix = self.remove(key)
        if ix < 0:
            return default
        return item(self.values, ix)

    def pop_or_none(self, key: K) -> V | None:
        ix = self.remove(key)
        if ix < 0:
            return None
        return item(self.values, ix)

    def copy(self) -> Dict[K, V]:
        """A new `dict` with the items of this one. A table with few removed entries goes to
        the copy as it is, and the copy of another one has a new table, as in CPython."""
        new: Dict[K, V] = Dict()
        if self.used == 0:
            return new
        if self.used >= len(self.keys) * 2 // 3:
            new.indices = self.indices.copy()
            new.hashes = self.hashes.copy()
            new.keys = self.keys.copy()
            new.values = self.values.copy()
            new.live = self.live.copy()
            new.used = self.used
            new.usable = self.usable
            new.unicode = self.unicode
            return new
        n = self.used
        if n > 5:
            new.resize((n * 3 + 1) // 2, self.unicode)
        i = 0
        while i < len(self.keys):
            if item(self.live, i):
                new.insert(item(self.keys, i), item(self.hashes, i), item(self.values, i))
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
        while i < len(self.keys):
            if item(self.live, i):
                key = item(self.keys, i)
                value = item(self.values, i)
                parts.append(repr(key) + ": " + repr(value))
            i += 1
        return "{" + ", ".join(parts) + "}"

    def clear(self) -> None:
        indices: list[int] = []
        hashes: list[Hash] = []
        keys: list[K] = []
        values: list[V] = []
        live: list[bool] = []
        self.indices = indices
        self.hashes = hashes
        self.keys = keys
        self.values = values
        self.live = live
        self.used = 0
        self.usable = 0
        self.unicode = True

    def iter(self) -> DictIter[K, V]:
        return DictIter(self)

    def pend(self, key: K, value: V) -> Dict[K, V]:
        """Keeps an item of a literal of `dict`, which [`done`] puts in the `dict` once each key
        and each value of the literal has its value, as `BUILD_MAP` of CPython does."""
        self.keys.append(key)
        self.values.append(value)
        return self

    def done(self) -> Dict[K, V]:
        """Puts in the `dict` the items that `pend` kept, in a table that CPython sizes for them,
        and gives the `dict`."""
        keys = self.keys
        values = self.values
        n = len(keys)
        self.clear()
        if n > 5:
            unicode = True
            for i in range(n):
                if not isinstance(item(keys, i), str):
                    unicode = False
                    break
            self.resize(min((n * 3 + 1) // 2, 131072), unicode)
        for i in range(n):
            key = item(keys, i)
            self.insert(key, hash_of(key), item(values, i))
        return self

    def find(self, key: K, h: Hash) -> int:
        """The index of the entry of `key`, whose `hash` is `h`, or -1 when the `dict` has no such
        key. The key of the entry is on the left of `==`. A comparison that changes the table or
        removes the entry starts the search again, as in CPython."""
        indices = self.indices
        size = len(indices)
        if size == 0:
            return -1
        mask = size - 1
        i = hash_slot(h, mask)
        perturb = h
        while True:
            ix = item(indices, i)
            if ix >= 0:
                k = item(self.keys, ix)
                if same(k, key):
                    return ix
                if hash_eq(item(self.hashes, ix), h):
                    equal = k == key
                    if not same(self.indices, indices) or not item(self.live, ix):
                        return self.find(key, h)
                    if equal:
                        return ix
            elif ix == -1:
                return -1
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % size

    def remove(self, key: K) -> int:
        """Removes the entry of `key` and gives its index, or -1 when the `dict` has no such key.
        The value of the entry stays in its place. An empty `dict` takes no `hash` of the key, as
        in CPython."""
        if self.used == 0:
            return -1
        h = hash_of(key)
        ix = self.find(key, h)
        if ix >= 0:
            set_item(self.indices, self.slot_of(h, ix), -2)
            set_item(self.live, ix, False)
            self.used -= 1
        return ix

    def slot_of(self, h: Hash, ix: int) -> int:
        """The slot of the entry at `ix`, whose `hash` is `h`."""
        indices = self.indices
        size = len(indices)
        mask = size - 1
        i = hash_slot(h, mask)
        perturb = h
        while item(indices, i) != ix:
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % size
        return i

    def empty_slot(self, h: Hash) -> int:
        """The first slot for `h` that holds no entry."""
        indices = self.indices
        size = len(indices)
        mask = size - 1
        i = hash_slot(h, mask)
        perturb = h
        while item(indices, i) >= 0:
            perturb = hash_shift(perturb)
            i = (i * 5 + 1 + hash_slot(perturb, mask)) % size
        return i

    def insert(self, key: K, h: Hash, value: V) -> None:
        """Puts `value` under `key`, whose `hash` is `h`. A key that the `dict` has keeps its first
        object."""
        if len(self.indices) == 0:
            self.new_table(8, isinstance(key, str))
        else:
            ix = self.find(key, h)
            if ix >= 0:
                set_item(self.values, ix, value)
                return
            if self.unicode and not isinstance(key, str):
                self.resize(self.used * 3, False)
            if self.usable <= 0:
                self.resize(self.used * 3, True)
        set_item(self.indices, self.empty_slot(h), len(self.keys))
        self.hashes.append(h)
        self.keys.append(key)
        self.values.append(value)
        self.live.append(True)
        self.usable -= 1
        self.used += 1

    def new_table(self, size: int, unicode: bool) -> None:
        """Gives the `dict` an empty table of `size` slots, without entries."""
        indices: list[int] = []
        list_reserve(indices, size)
        for i in range(size):
            set_item(indices, i, -1)
        list_set_len(indices, size)
        hashes: list[Hash] = []
        keys: list[K] = []
        values: list[V] = []
        live: list[bool] = []
        self.indices = indices
        self.hashes = hashes
        self.keys = keys
        self.values = values
        self.live = live
        self.usable = size * 2 // 3
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
        self.new_table(size, unicode and self.unicode)
        for i in range(len(keys)):
            if item(live, i):
                h = item(hashes, i)
                set_item(self.indices, self.empty_slot(h), len(self.keys))
                self.hashes.append(h)
                self.keys.append(item(keys, i))
                self.values.append(item(values, i))
                self.live.append(True)
        self.usable -= len(self.keys)


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
        n = len(d.keys)
        while i < n and not item(d.live, i):
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
        return item(self.d.keys, self.index)

    def value(self) -> V:
        return item(self.d.values, self.index)
