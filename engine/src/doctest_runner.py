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


def _parse(doc, name):
    """The examples of *doc* as ``(source, want)`` pairs.

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
        if not _is_blank_or_comment(source):
            if not source.endswith("\n"):
                source += "\n"
            if want and not want.endswith("\n"):
                want += "\n"
            examples.append((source, want))
    return examples


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
    pairs."""
    _groups = []
    for _name in sorted(dir(module)):
        _obj = getattr(module, _name)
        if not callable(_obj):
            continue
        if getattr(_obj, "__module__", None) != module.__name__:
            continue
        _doc = getattr(_obj, "__doc__", None)
        if not _doc or ">>>" not in _doc:
            continue
        _groups.append((_name, _doc))
    return _groups


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
        for _src, _want in _examples:
            _src = _src.rstrip("\n")
            _expected = _want[:-1]
            _total += 1
            _old_stdout = _sys.stdout
            _sys.stdout = _buf = _Capture()
            _had_error = False
            try:
                _code = compile(_src, "<doctest>", "eval")
                _result = eval(_code, vars(module))
                if _result is not None:
                    print(repr(_result))
            except SyntaxError:
                try:
                    _code = compile(_src, "<doctest>", "exec")
                    exec(_code, vars(module))
                except BaseException as _e:
                    _had_error = True
                    print(type(_e).__name__ + ": " + str(_e))
            except BaseException as _e:
                _had_error = True
                print(type(_e).__name__ + ": " + str(_e))
            finally:
                _sys.stdout = _old_stdout
            _got = _buf.getvalue().rstrip("\n")
            if _got != _expected:
                if _had_error:
                    _errors += 1
                    _write_err(
                        "    Error in example:\n"
                        "      " + _src.replace("\n", "\n      ") + "\n"
                        "    Got:\n"
                        "      " + _got.replace("\n", "\n      ") + "\n"
                    )
                else:
                    _failed += 1
                    _write_err(
                        "    Failed example:\n"
                        "      " + _src.replace("\n", "\n      ") + "\n"
                        "    Expected:\n"
                        "      " + _expected.replace("\n", "\n      ") + "\n"
                        "    Got:\n"
                        "      " + _got.replace("\n", "\n      ") + "\n"
                    )
            elif verbose:
                _write_err("    ok: " + _src.split("\n")[0] + "\n")
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

    _sys.path.insert(0, _directory_of(path))
    module = types.ModuleType("__doctest__")
    module.__file__ = path
    exec(compile(source, path, "exec"), vars(module))
    return run_doctests(module, verbose=verbose)


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
