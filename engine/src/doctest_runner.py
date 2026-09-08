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


def _extract_examples(doc):
    """Yield (source_lines, expected) pairs from a docstring."""
    lines = doc.expandtabs().splitlines()
    i = 0
    while i < len(lines):
        stripped = lines[i].lstrip()
        if stripped.startswith(">>> "):
            src = [stripped[4:]]
            i += 1
            while i < len(lines):
                cont = lines[i].lstrip()
                if cont.startswith("... "):
                    src.append(cont[4:])
                    i += 1
                else:
                    break
            expected_lines = []
            while i < len(lines):
                el = lines[i].lstrip()
                if el == "" or el.startswith(">>> "):
                    break
                expected_lines.append(el)
                i += 1
            yield "\n".join(src), "\n".join(expected_lines)
        else:
            i += 1


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


def _collect_examples(module):
    """Group the doctests of *module* as (function name, examples) pairs.

    Functions without examples are left out, so an empty result means there is
    nothing to run.
    """
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
        _examples = []
        for _example in _extract_examples(_doc):
            _examples.append(_example)
        if _examples:
            _groups.append((_name, _examples))
    return _groups


def run_doctests(module, verbose=False):
    """Run doctests found in *module* and return the number of failures."""
    _groups = _collect_examples(module)
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
        for _src, _expected in _examples:
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
