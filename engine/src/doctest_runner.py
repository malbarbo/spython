"""Minimal doctest runner that avoids the stdlib ``doctest`` module.

The stdlib module requires ``inspect`` -> ``dis`` -> ``io`` -> ``_io.FileIO``,
which is not available in the RustPython WASM build.  This runner only depends
on ``sys`` and builtins so it works on every target.

The public entry point is ``run_doctests(module)``, which returns the number of
failed examples, and which is what the REPL calls on ``__main__``.  A whole
file goes through ``_run_file(path, source)``: the ``test`` command drives
``_main(sys.argv[1:])``, which reads the files it names, and a page, which has
no files to read, drives ``_run_file`` with the text it already holds.
"""

import sys as _sys


class _Capture:
    """File-like object that collects writes into a list."""

    def __init__(self):
        self._parts = []

    def write(self, s):
        self._parts.append(str(s))

    def flush(self):
        pass

    def getvalue(self):
        return "".join(self._parts)


# The rules of `DocTestParser` in the stdlib `doctest`, written without `re`,
# which the binary does not freeze.


def _indent_of(line):
    """The number of spaces at the start of *line*."""
    return len(line) - len(line.lstrip(" "))


def _ps1_indent(line):
    """The indentation of *line* if it starts with ``>>>``, ``None`` otherwise."""
    n = _indent_of(line)
    return n if line.startswith(">>>", n) else None


def _min_indent(lines):
    """The smallest indentation of a line that is not blank."""
    indents = []
    for line in lines:
        n = _indent_of(line)
        if n < len(line) and not line[n].isspace():
            indents.append(n)
    return min(indents) if indents else 0


def _is_blank_or_comment(source):
    """Returns ``True`` if *source* is one line that is blank or only a comment,
    ``False`` otherwise."""
    if source.endswith("\n"):
        source = source[:-1]
    if "\n" in source:
        return False
    rest = source.lstrip(" ")
    return rest == "" or rest.startswith("#")


def _check_prefix(lines, prefix, name, lineno):
    for i, line in enumerate(lines):
        if line and not line.startswith(prefix):
            raise ValueError(
                "line %r of the docstring for %s has inconsistent leading whitespace: %r"
                % (lineno + i + 1, name, line)
            )


def _exc_msg(want):
    """The exception that *want* expects, which is everything from the first
    line that starts with a letter, a digit, or ``_`` after a ``Traceback``
    header, and ``None`` for a *want* with no such header."""
    lines = want.split("\n")
    rest = None
    for header in _TRACEBACK_HEADERS:
        if lines[0].startswith(header):
            rest = lines[0][len(header) :]
    if rest is None or rest.strip() != "":
        return None
    for i in range(1, len(lines)):
        c = lines[i][:1]
        if c and (c.isalnum() or c == "_"):
            return "\n".join(lines[i:])
    return None


_TRACEBACK_HEADERS = ("Traceback (most recent call last):", "Traceback (innermost last):")


def _parse(doc, name):
    """The examples of *doc* as ``(source, want, exc_msg)`` triples, where
    *exc_msg* is what :func:`_exc_msg` reads off *want*.

    Raises ``ValueError`` for a prompt with no space after it and for a line
    that does not line up with the ``>>>`` of its example, with the message of
    the stdlib.
    """
    lines = doc.expandtabs().split("\n")
    margin = _min_indent(lines)
    if margin > 0:
        lines = [line[margin:] for line in lines]
    examples = []
    i = 0
    while i < len(lines):
        indent = _ps1_indent(lines[i])
        if indent is None:
            i += 1
            continue
        start = i
        source_lines = [lines[i]]
        i += 1
        while i < len(lines) and lines[i].lstrip(" ").startswith("..."):
            source_lines.append(lines[i])
            i += 1
        want_lines = []
        while i < len(lines) and lines[i].strip(" ") != "" and _ps1_indent(lines[i]) is None:
            want_lines.append(lines[i])
            i += 1
        for k, line in enumerate(source_lines):
            if len(line) >= indent + 4 and line[indent + 3] != " ":
                raise ValueError(
                    "line %r of the docstring for %s lacks blank after %s: %r"
                    % (start + k + 1, name, line[indent : indent + 3], line)
                )
        _check_prefix(source_lines[1:], " " * indent + ".", name, start)
        _check_prefix(want_lines, " " * indent, name, start + len(source_lines))
        source = "\n".join(line[indent + 4 :] for line in source_lines)
        want = "\n".join(line[indent:] for line in want_lines)
        exc_msg = _exc_msg(want)
        if not _is_blank_or_comment(source):
            if not source.endswith("\n"):
                source += "\n"
            if want and not want.endswith("\n"):
                want += "\n"
            if exc_msg is not None and not exc_msg.endswith("\n"):
                exc_msg += "\n"
            examples.append((source, want, exc_msg))
    return examples


_BLANKLINE = "<BLANKLINE>"


def _to_ascii(s):
    return str(s.encode("ASCII", "backslashreplace"), "ASCII")


def _check_output(want, got):
    """Returns ``True`` if *got* is the output that *want* asks for, ``False``
    otherwise.

    The rules are those of ``OutputChecker`` in the stdlib with no options.
    A ``<BLANKLINE>`` in *want* stands for a blank line, which would end the
    example, and a line of *got* that holds only spaces counts as blank.
    """
    got = _to_ascii(got)
    want = _to_ascii(want)
    if got == want:
        return True
    want = "\n".join(
        "" if line.startswith(_BLANKLINE) and line[len(_BLANKLINE) :].strip() == "" else line
        for line in want.split("\n")
    )
    got = "\n".join("" if line and line.isspace() else line for line in got.split("\n"))
    return got == want


def _shown(output):
    """*output* as the report shows it, with ``<BLANKLINE>`` for a blank line
    that would otherwise not show."""
    lines = output.split("\n")
    for i in range(len(lines) - 1):
        if lines[i].strip(" ") == "":
            lines[i] = _BLANKLINE
    return "\n".join(lines).removesuffix("\n")


def _write_err(text):
    """Write *text* to stderr and flush it right away.

    Flushing matters because the messages are progress reports: if an example
    loops forever, the student must still see that the tests started.
    """
    _sys.stderr.write(text)
    _sys.stderr.flush()


def _plural(n, word):
    if n == 1:
        return "1 " + word
    return str(n) + " " + word + ("es" if word == "success" else "s")


def _collect_docstrings(module):
    """The docstrings of *module* that hold a ``>>>``, as (name, docstring)
    pairs, the docstring of the module first and the others in the order of
    their names.

    The rules are those of ``DocTestFinder`` in the stdlib. The docstrings
    come from the module, from the functions and classes it defines, and
    from the methods, classes, and properties those classes define, each
    object once.
    """
    found = []
    seen = set()

    def visit(obj, name):
        if id(obj) in seen:
            return
        seen.add(id(obj))
        doc = getattr(obj, "__doc__", None)
        if doc is not None and not isinstance(doc, str):
            doc = str(doc)
        if doc and ">>>" in doc:
            found.append((name, doc))
        if obj is module:
            for valname, val in vars(module).items():
                if (_is_routine(_unwrap(val)) or isinstance(val, type)) and _from_module(module, val):
                    visit(val, valname)
        elif isinstance(obj, type):
            for valname, val in vars(obj).items():
                if isinstance(val, (staticmethod, classmethod)):
                    val = val.__func__
                if (
                    _is_routine(val) or isinstance(val, type) or isinstance(val, property)
                ) and _from_module(module, val):
                    visit(val, name + "." + valname)

    visit(module, "<module>")
    # No function or class can have that name, so the key puts the module
    # first and leaves the others in the order of their names.
    found.sort(key=lambda pair: (pair[0] != "<module>", pair[0]))
    return found


# The types of `inspect.isroutine`, which needs more of the stdlib than the
# binary freezes.
_FunctionType = type(_indent_of)
_BuiltinFunctionType = type(len)
_MethodType = type(_Capture().write)
_MethodWrapperType = type(object().__str__)


def _unwrap(obj):
    """The function under the wrappers that ``functools.wraps`` made."""
    seen = set()
    while hasattr(obj, "__wrapped__") and id(obj) not in seen:
        seen.add(id(obj))
        obj = obj.__wrapped__
    return obj


def _is_method_descriptor(obj):
    if isinstance(obj, (type, _MethodType, _FunctionType)):
        return False
    t = type(obj)
    return hasattr(t, "__get__") and not hasattr(t, "__set__") and not hasattr(t, "__delete__")


def _is_routine(obj):
    """Returns ``True`` if *obj* is a function or a method, ``False``
    otherwise."""
    return isinstance(
        obj, (_FunctionType, _BuiltinFunctionType, _MethodType, _MethodWrapperType)
    ) or _is_method_descriptor(obj)


def _from_module(module, obj):
    """Returns ``True`` if *module* defines *obj*, ``False`` otherwise.

    The answer comes from the name of the module that *obj* carries, as
    ``DocTestFinder`` decides for a module in ``sys.modules``. The module of a
    test run is not there, and the wrapper that a decorator from another
    module returns has the globals of that module.
    """
    name = getattr(obj, "__module__", None)
    if name is not None:
        return name == module.__name__
    if hasattr(obj, "__objclass__"):
        return obj.__objclass__.__module__ == module.__name__
    return True


def _run_example(source, module):
    """Run *source* in *module* as the interactive prompt runs one statement.

    Returns what it printed and, for an example that raised, the line a
    traceback ends with, or ``None``.
    """
    old_stdout = _sys.stdout
    old_displayhook = _sys.displayhook
    _sys.stdout = buf = _Capture()
    # The repr of a value, as the stdlib shows it, and not the picture that
    # the displayhook of the REPL draws.
    _sys.displayhook = _sys.__displayhook__
    exc_line = None
    try:
        exec(compile(source, "<doctest>", "single"), vars(module))
    except KeyboardInterrupt:
        # The student stopped the run, which is not something the example did.
        raise
    except BaseException as e:
        exc_line = _exception_line(e, module)
    finally:
        _sys.stdout = old_stdout
        _sys.displayhook = old_displayhook
    return buf.getvalue(), exc_line


def _exception_line(e, module):
    """The lines a traceback of *e* ends with, as Python prints them when the
    file of *module* runs as the main program."""
    t = type(e)
    name = t.__qualname__
    if t.__module__ not in ("builtins", "__main__", module.__name__):
        name = t.__module__ + "." + name
    if isinstance(e, SyntaxError):
        value = str(e.msg)
    else:
        try:
            value = str(e)
        except BaseException:
            value = "<exception str() failed>"
    line = name + ": " + value + "\n" if value else name + "\n"
    for note in getattr(e, "__notes__", None) or []:
        line += str(note) + "\n"
    return line


def run_doctests(module, verbose=False):
    """Run doctests found in *module* and return the number of failures."""
    _groups = []
    for _name, _doc in _collect_docstrings(module):
        try:
            _examples = _parse(_doc, _name)
        except ValueError as _e:
            _examples = _e
        if _examples:
            _groups.append((_name, _examples))
    if not _groups:
        return 0
    # Announce the run before executing anything: an example that hangs must
    # not leave the student staring at an empty screen.
    _write_err("Running tests...\n")
    _failed = 0
    _errors = 0
    _total = 0
    for _func_name, _examples in _groups:
        # The name goes out before the examples run, so a test that never
        # returns still tells the student which function to look at.
        _write_err("  " + _func_name + "\n")
        if isinstance(_examples, ValueError):
            _total += 1
            _errors += 1
            _write_err("    " + str(_examples) + "\n")
            continue
        for _src, _want, _exc_msg in _examples:
            _total += 1
            _got, _exc_line = _run_example(_src, module)
            _src = _src.rstrip("\n").replace("\n", "\n      ")
            if _exc_line is None:
                _ok = _check_output(_want, _got)
            elif _exc_msg is not None:
                # An example that names its exception after a `Traceback`
                # header passes on that exception, whatever it printed.
                _got = _exc_line
                _ok = _check_output(_exc_msg, _got)
            else:
                # With no header, the exception is one more line of output,
                # as a traceback ends.
                _got += _exc_line
                _ok = _check_output(_want, _got)
            _got = _shown(_got).replace("\n", "\n      ")
            if _ok:
                if verbose:
                    _write_err("    ok: " + _src.split("\n")[0] + "\n")
            elif _exc_line is not None and _exc_msg is None:
                _errors += 1
                _write_err("    Error in example:\n      " + _src + "\n    Got:\n      " + _got + "\n")
            else:
                _failed += 1
                _write_err(
                    "    Failed example:\n      " + _src + "\n"
                    "    Expected:\n      "
                    + _want.removesuffix("\n").replace("\n", "\n      ")
                    + "\n    Got:\n      " + _got + "\n"
                )
    _successes = _total - _failed - _errors
    _write_err(
        _plural(_total, "test")
        + ", "
        + _plural(_successes, "success")
        + ", "
        + _plural(_failed, "failure")
        + " and "
        + _plural(_errors, "error")
        + ".\n"
    )
    return _failed + _errors


def _directory_of(path):
    """The directory *path* sits in, which is where its imports resolve from.

    ``os.path`` would say this, but the wasm build has not got ``os``.
    """
    cut = max(path.rfind("/"), path.rfind("\\"))
    if cut < 0:
        return "."
    return path[:cut] or "/"


def _run_file(path, source, verbose=False):
    """Run the doctests of *source*, which is what the file *path* holds.

    The source runs in a module of its own, so that a ``__main__`` guard in it
    stays shut, and with its own directory ahead of the path, so that its
    imports resolve the way they do when the file runs on its own.

    ``types`` is imported here and not at the top of the file to keep the top
    to ``sys`` and builtins, which is what makes the runner work on every
    target.
    """
    import types

    # A file sees only the modules of its own directory, and not those that
    # the file before it imported under the same name.
    modules = dict(_sys.modules)
    path_before = list(_sys.path)
    try:
        _sys.path.insert(0, _directory_of(path))
        module = types.ModuleType("__doctest__")
        module.__file__ = path
        exec(compile(source, path, "exec"), vars(module))
        return run_doctests(module, verbose=verbose)
    finally:
        for name in list(_sys.modules):
            if name not in modules:
                del _sys.modules[name]
        _sys.modules.update(modules)
        _sys.path[:] = path_before


def _main(argv):
    """Run the doctests of the files named in *argv*, one module each.

    A leading ``--verbose`` reports the examples that pass as well, and with
    more than one file each name is printed before its tests.  The return
    value is the exit status: the number of examples that did not pass.
    """
    verbose = bool(argv) and argv[0] == "--verbose"
    if verbose:
        argv = argv[1:]
    total = 0
    for path in argv:
        if len(argv) > 1:
            print(path, flush=True)
        with open(path) as f:
            source = f.read()
        total += _run_file(path, source, verbose)
    return total
