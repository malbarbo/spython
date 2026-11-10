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
