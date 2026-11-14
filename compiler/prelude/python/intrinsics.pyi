"""What the prelude asks of the compiler. Each function is an instruction of the module."""

from typing import NoReturn, Protocol, Self

class StrBuilder:
    """A `str` that the prelude still writes, one code point at a time."""

    def __setitem__(self, index: int, code_point: int) -> None: ...

class Ordered(Protocol):
    """A value that `<` compares with a value of its type, which `sort`, `min` and `max`
    take."""

    def __lt__(self, other: Self, /) -> bool: ...

class Hash:
    """The `hash` of a value, an integer of 64 bits that only the prelude holds."""

def hash_of[T](x: T) -> Hash:
    """The `hash` of `x`, as CPython gives it with `PYTHONHASHSEED=0`."""

def hash_slot(h: Hash, mask: int) -> int:
    """The bits of `h` that `mask` keeps, where `mask` is 2^k - 1 for a `k` below 31."""

def hash_shift(h: Hash) -> Hash:
    """`h` shifted right by 5 bits, with zeros in the 5 high bits, as the probe of a table of
    CPython shifts it."""

def hash_eq(a: Hash, b: Hash) -> bool:
    """Returns `True` if `a` and `b` are the same `hash`, `False` otherwise."""

def exact[T](x: T, other: object) -> T | None:
    """`other` as a value of the type of `x`, an object, when the class of `other` is exactly the
    class of `x`, and `None` otherwise. `x` has no effect."""

def in_repr[T](x: T) -> bool:
    """Returns `True` if the `repr` of `x`, an object, runs, `False` otherwise."""

def during_repr[T](x: T, text: str) -> str:
    """`text`, which the program evaluates with `x`, an object, among the objects whose `repr`
    runs."""

def same[T](a: T, b: T) -> bool:
    """Returns `True` if `a` and `b` are the same object, `False` otherwise. A value of a type
    without a reference, as an `int`, is never the same object. Neither argument has an effect."""

def builder_new(length: int) -> StrBuilder:
    """A builder of `length` code points, each of them 0."""

def builder_done(builder: StrBuilder) -> str:
    """The `str` of the builder, which nothing writes from here on."""

def code_point(s: str, index: int) -> int:
    """The code point of `s` at `index`, which is not negative and is below `len(s)`."""

def is_printable(code_point: int) -> bool:
    """Returns `True` if `str.isprintable` accepts the code point, `False` otherwise."""

def write_err(text: str) -> None:
    """Writes `text` to the standard error, and flushes it."""

def captured_output() -> str:
    """What the last example of a docstring wrote to the standard output."""

def captured_exception() -> str:
    """The last line of the traceback of the exception that ended the last example of a
    docstring, with its line feed. An example that raised nothing gives an empty `str`."""

def upper_at(code_point: int, index: int) -> int:
    """The code point at `index` of `str.upper` of the code point, or -1 past its end."""

def lower_at(code_point: int, index: int) -> int:
    """The code point at `index` of `str.lower` of the code point, or -1 past its end. A capital
    sigma gives the small sigma that is not the final one."""

def is_cased(code_point: int) -> bool:
    """Returns `True` if the code point has the property `Cased` of Unicode,
    `False` otherwise."""

def is_case_ignorable(code_point: int) -> bool:
    """Returns `True` if the code point has the property `Case_Ignorable` of Unicode,
    `False` otherwise."""

def natural_power(base: int, exp: int) -> int:
    """`base ** exp` for an `exp` that is not negative. A power outside 31 bits stops the program."""

def mul_mod(a: int, b: int, m: int) -> int:
    """`a * b % m` for an `m` that is not 0, without a product outside 31 bits."""

def title_at(code_point: int, index: int) -> int:
    """The code point at `index` of the title case of the code point, or -1 past its end."""

def is_uppercase(code_point: int) -> bool:
    """Returns `True` if the code point has the property `Uppercase` of Unicode,
    `False` otherwise."""

def is_lowercase(code_point: int) -> bool:
    """Returns `True` if the code point has the property `Lowercase` of Unicode,
    `False` otherwise."""

def fold_at(code_point: int, index: int) -> int:
    """The code point at `index` of `str.casefold` of the code point, or -1 past its end."""

def is_alpha(code_point: int) -> bool:
    """Returns `True` if `str.isalpha` accepts the code point, `False` otherwise."""

def is_decimal(code_point: int) -> bool:
    """Returns `True` if `str.isdecimal` accepts the code point, `False` otherwise."""

def is_digit(code_point: int) -> bool:
    """Returns `True` if `str.isdigit` accepts the code point, `False` otherwise."""

def is_numeric(code_point: int) -> bool:
    """Returns `True` if `str.isnumeric` accepts the code point, `False` otherwise."""

def is_xid_start(code_point: int) -> bool:
    """Returns `True` if the code point has the property `XID_Start` of Unicode,
    `False` otherwise."""

def is_xid_continue(code_point: int) -> bool:
    """Returns `True` if the code point has the property `XID_Continue` of Unicode,
    `False` otherwise."""

def stop(text: str) -> NoReturn:
    """Stops the program with the error of spython of `text`, for what CPython does and spython
    cannot."""

def decimal_value(code_point: int) -> int:
    """The value of the code point as a decimal digit, or -1 when `str.isdecimal` does not accept it."""

def parse_float(text: str) -> float:
    """The `float` of `text`, which is ASCII in the form of a float of Python without underscores."""

def list_capacity[T](xs: list[T]) -> int:
    """How many items the array of `xs` has room for."""

def list_reserve[T](xs: list[T], capacity: int) -> None:
    """Gives `xs` an array with room for `capacity` items, which is not below `len(xs)`, with the
    items of `xs`."""

def list_set_len[T](xs: list[T], n: int) -> None:
    """Makes `xs` a list of its first `n` items, where `n` is not above the capacity of `xs`. An
    item past the old length has the value that the prelude set there."""

def list_truncate[T](xs: list[T], n: int) -> None:
    """Makes `xs` a list of its first `n` items, where `n` is not above `len(xs)`, and drops the
    items past them, so they may go."""

def set_item[T](xs: list[T], index: int, x: T) -> None:
    """Sets the item at `index` of the array of `xs`, which is not negative and is below the
    capacity of `xs`, to `x`."""

def item[T](xs: list[T], index: int) -> T:
    """The item of `xs` at `index`, which is not negative and is below `len(xs)`."""

def copy_items[T](dst: list[T], at: int, src: list[T], start: int, count: int) -> None:
    """Copies `count` items of `src`, from `start`, to the array of `dst`, from `at`, as if through
    a buffer, so the two ranges may overlap. Both ranges are below the capacity of their lists."""
