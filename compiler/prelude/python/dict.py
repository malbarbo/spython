"""The class of `dict`, whose copy for the types of the keys and of the values is the class of a
`dict` of the program."""


class Dict[K, V]:
    keys: list[K]
    values: list[V]

    def __init__(self) -> None:
        keys: list[K] = []
        values: list[V] = []
        self.keys = keys
        self.values = values

    def __len__(self) -> int:
        return len(self.keys)

    def clear(self) -> None:
        keys: list[K] = []
        values: list[V] = []
        self.keys = keys
        self.values = values
