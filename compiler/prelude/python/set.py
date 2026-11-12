"""The class of `set`, whose copy for the type of the items is the class of a `set` of the
program."""


class Set[T]:
    items: list[T]

    def __init__(self) -> None:
        items: list[T] = []
        self.items = items

    def __len__(self) -> int:
        return len(self.items)

    def clear(self) -> None:
        items: list[T] = []
        self.items = items
