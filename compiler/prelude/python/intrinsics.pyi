"""What the prelude asks of the compiler. Each function is an instruction of the module."""

class StrBuilder:
    """A `str` that the prelude still writes, one code point at a time."""

    def __setitem__(self, index: int, code_point: int) -> None: ...

def builder_new(length: int) -> StrBuilder:
    """A builder of `length` code points, each of them 0."""

def builder_done(builder: StrBuilder) -> str:
    """The `str` of the builder, which nothing writes from here on."""

def code_point(s: str, index: int) -> int:
    """The code point of `s` at `index`, which is not negative and is below `len(s)`."""

def is_printable(code_point: int) -> bool:
    """Whether `str.isprintable` accepts the code point."""

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
    """Whether the code point has the property `Cased` of Unicode."""

def is_case_ignorable(code_point: int) -> bool:
    """Whether the code point has the property `Case_Ignorable` of Unicode."""

def natural_power(base: int, exp: int) -> int:
    """`base ** exp` for an `exp` that is not negative. A power outside 31 bits stops the program."""

def title_at(code_point: int, index: int) -> int:
    """The code point at `index` of the title case of the code point, or -1 past its end."""

def is_uppercase(code_point: int) -> bool:
    """Whether the code point has the property `Uppercase` of Unicode."""

def is_lowercase(code_point: int) -> bool:
    """Whether the code point has the property `Lowercase` of Unicode."""

def fold_at(code_point: int, index: int) -> int:
    """The code point at `index` of `str.casefold` of the code point, or -1 past its end."""

def is_alpha(code_point: int) -> bool:
    """Whether `str.isalpha` accepts the code point."""

def is_decimal(code_point: int) -> bool:
    """Whether `str.isdecimal` accepts the code point."""

def is_digit(code_point: int) -> bool:
    """Whether `str.isdigit` accepts the code point."""

def is_numeric(code_point: int) -> bool:
    """Whether `str.isnumeric` accepts the code point."""

def is_xid_start(code_point: int) -> bool:
    """Whether the code point has the property `XID_Start` of Unicode."""

def is_xid_continue(code_point: int) -> bool:
    """Whether the code point has the property `XID_Continue` of Unicode."""
