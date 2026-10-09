"""Minimal doctest runner that avoids the stdlib ``doctest`` module.

The stdlib module requires ``inspect`` -> ``dis`` -> ``io`` -> ``_io.FileIO``,
which is not available in the RustPython WASM build.  This runner only depends
on ``sys`` and builtins so it works on every target.

The public entry point is ``run_doctests(module, source)``, which returns the
number of failed examples, and which is what the REPL calls on ``__main__``.  A whole
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


def _docstrings(source):
    """The docstrings of *source*, as (name, docstring) pairs, the docstring
    of the module first and the others in the order of their names.

    The docstrings come from the extraction of the checker, so that the
    examples that run are the ones the checker has read, whatever a
    decorator or a later assignment does to the objects.
    """
    from _spython_ffi import doctest_docstrings

    found = doctest_docstrings(source)
    # No function or class can have that name, so the key puts the module
    # first and leaves the others in the order of their names.
    found.sort(key=lambda pair: (pair[0] != "<module>", pair[0]))
    return found


def _run_example(source, globs, module):
    """Run *source* in *globs*, a copy of the globals of *module*, as the
    interactive prompt runs one statement.

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
        exec(compile(source, "<doctest>", "single"), globs)
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


def run_doctests(module, source, verbose=False):
    """Run the doctests of *source* in *module*, where *source* has run, and
    return the number of failures."""
    _groups = []
    for _name, _doc in _docstrings(source):
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
        # The examples of a docstring share the names they define, and the
        # module, the other docstrings, and the session after them do not
        # see those names.
        _globs = dict(vars(module))
        for _src, _want, _exc_msg in _examples:
            _total += 1
            _got, _exc_line = _run_example(_src, _globs, module)
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
        _globs.clear()
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


def _run_file(path, source, verbose=False, directory=None):
    """Run the doctests of *source*, which is what the file *path* holds, and
    return the number of failures. A file that exits or raises before its
    tests run counts as one.

    The source runs in a module of its own, so that a ``__main__`` guard in it
    stays shut, and with *directory* ahead of the path, so that its imports
    resolve the way they do when the file runs on its own. Without
    *directory*, the directory of *path* goes there.

    ``types`` is imported here and not at the top of the file to keep the top
    to ``sys`` and builtins, which is what makes the runner work on every
    target.
    """
    import types

    from _spython_ffi import keep_source

    # A file sees only the modules of its own directory, and not those that
    # the file before it imported under the same name.
    modules = dict(_sys.modules)
    path_before = list(_sys.path)
    try:
        _sys.path.insert(0, directory or _directory_of(path))
        module = types.ModuleType("__doctest__")
        module.__file__ = path
        keep_source(path, source)
        try:
            exec(compile(source, path, "exec"), vars(module))
        except SystemExit as e:
            # A file that stops halfway leaves no tests to run, and a status
            # of 0 would say that they passed.
            if e.code is not None and not isinstance(e.code, int):
                _write_err(str(e.code) + "\n")
            _write_err("The file exited before its tests ran.\n")
            return 1
        except KeyboardInterrupt:
            raise
        except BaseException as e:
            # The traceback starts at the file, as `run` shows it, and not in
            # the frame of the runner that called `exec`.
            tb = e.__traceback__
            _sys.excepthook(type(e), e, tb.tb_next if tb else None)
            _write_err("The file raised an exception before its tests ran.\n")
            return 1
        return run_doctests(module, source, verbose=verbose)
    finally:
        for name in list(_sys.modules):
            if name not in modules:
                del _sys.modules[name]
        _sys.modules.update(modules)
        _sys.path[:] = path_before


def _main(argv):
    """Run the doctests of the files named in *argv*, one module each. Each
    name comes before the directory its imports resolve from.

    A leading ``--verbose`` reports the examples that pass as well, and with
    more than one file each name is printed before its tests.  The return
    value is the exit status: the number of examples that did not pass.
    """
    verbose = bool(argv) and argv[0] == "--verbose"
    if verbose:
        argv = argv[1:]
    files = list(zip(argv[::2], argv[1::2]))
    total = 0
    for path, directory in files:
        if len(files) > 1:
            print(path, flush=True)
        with open(path) as f:
            source = f.read()
        total += _run_file(path, source, verbose, directory)
    return total
