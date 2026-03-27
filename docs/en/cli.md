# Running a file

To run a Python file with type checking:

```sh
spython run file.py
```

Spython checks type annotations and allowed constructs for the teaching level before execution. If there are errors, execution is blocked and errors are displayed.

```python
# hello.py
def main() -> None:
    print("Hello world!")

main()
```

```sh
$ spython run --level 5 hello.py
Hello world!
```


## Teaching levels

Spython restricts which Python constructs are available based on the teaching level. The default level is 0 (most restricted).

| Level | Name       | Added constructs                                                      |
|-------|------------|-----------------------------------------------------------------------|
| 0     | Functions  | `def`, `return`, scalars, `str` indexing                              |
| 1     | Selection  | `if`/`elif`/`else`                                                    |
| 2     | User types | `class` (Enum / `@dataclass`), `match`                                |
| 3     | Repetition | `list` literals, `for`, `while`, `+=`                                 |
| 4     | Classes    | `class` with methods, `dict`/`set`, comprehensions, `lambda`          |
| 5     | Full       | unrestricted (only annotations are still required)                    |

To specify the level:

```sh
spython run --level 2 file.py
```

For example, at level 0, using `if` produces an error:

```python
# cond.py
def f(x: int) -> int:
    if x > 0:
        return x
    return 0
```

```sh
$ spython run --level 0 cond.py
error[forbidden-selection]: `if` statement is not allowed at level 0 (Functions)
```


## Required annotations

At all levels, spython requires complete type annotations:

- Every function parameter (except `self`/`cls`) must have a type annotation
- Every function must have a return type annotation
- Every class-body variable assignment must have a type annotation

```python
# no_annotation.py
def double(x):
    return x * 2
```

```sh
$ spython run no_annotation.py
error[missing-parameter-annotation]: Parameter `x` is missing a type annotation
error[missing-return-annotation]: Function `double` is missing a return type annotation
```

The correct version:

```python
# double.py
def double(x: int) -> int:
    return x * 2

print(double(5))
```

```sh
$ spython run --level 5 double.py
10
```


# Interactive mode (REPL)

To enter interactive mode:

```sh
spython
```

In the REPL, you can type expressions and definitions:

```
>>> 1 + 2
3
>>> x: int = 10
>>> x * 2
20
```

The REPL checks types and annotations on each input. Definitions without annotations are rejected:

```
>>> def f(x): return x
error[missing-parameter-annotation]: Parameter `x` is missing a type annotation
error[missing-return-annotation]: Function `f` is missing a return type annotation
```

The correct version:

```
>>> def f(x: int) -> int:
...     return x * 2
...
>>> f(5)
10
```

To specify the teaching level in the REPL:

```sh
spython repl --level 3
```

You can also load a file, making its definitions available in the REPL.
For example, given the file `double.py`:

```python
def double(x: int) -> int:
    """
    >>> double(0)
    0
    >>> double(3)
    6
    """
    return x * 2
```

You can use the `double` function in the REPL:

```sh
spython repl --level 5 double.py
```

```
>>> double(5)
10
>>> double(3) + 1
7
```

The file is checked (types and doctests) before being loaded.


## REPL commands

`:help` — Shows available commands.

`:type` — Shows the static type of an expression without evaluating it:

```
>>> :type 1 + 2
int
>>> :type [1, 2, 3]
list[int]
>>> :type "hello"
str
```

`:level` — Shows or changes the teaching level:

```
>>> :level
level 0 - Functions
>>> :level 3
level 3 - Repetition
```

If the code already entered is not compatible with the new level, the change is rejected.

`:theme` — Shows or changes the syntax highlighting theme (`light` or `dark`):

```
>>> :theme
dark
>>> :theme light
light
```

The theme and level preferences are saved to the user's configuration directory.

`:quit` — Exits the REPL (or `Ctrl+d`).


# Tests (doctests)

To run the doctests of a file:

```sh
spython check file.py
```

Tests are written as doctests in docstrings:

```python
# test.py
def double(x: int) -> int:
    """
    >>> double(0)
    0
    >>> double(3)
    6
    """
    return x * 2
```

```sh
$ spython check --level 5 test.py
Running tests...
2 tests, 2 successes, 0 failures and 0 errors.
```

Use `--verbose` to see all tests (not just failures):

```sh
spython check --level 5 --verbose test.py
```


# Formatting

To format Python files:

```sh
spython format file.py
```

Or to format all `.py` files in a directory:

```sh
spython format directory/
```


# Commands

| Command | Description |
|---------|-------------|
| `spython` | Interactive mode (REPL) at level 0 |
| `spython repl [file]` | Interactive mode (REPL) |
| `spython run file` | Run the file with type checking |
| `spython check files` | Run doctests |
| `spython format paths` | Format code |
| `spython help` | Show help |

# Options

| Option | Description |
|--------|-------------|
| `--level N` | Teaching level (0-5, default: 0) |
| `--verbose` | Show all tests (with `check`) |
| `--version` | Print version |
