"""Minimal doctest runner that avoids the stdlib ``doctest`` module.

The stdlib module requires ``inspect`` -> ``dis`` -> ``io`` -> ``_io.FileIO``,
which is not available in the RustPython WASM build.  This runner only depends
on ``sys`` and builtins so it works on every target.

The public entry point is ``run_doctests(module)`` which returns the number of
failed examples.  When this file is executed as a script (via ``include_str!``
in ``repl_new``), it tests ``__main__`` automatically.
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


def _plural(n, word):
    if n == 1:
        return "1 " + word
    return str(n) + " " + word + ("es" if word == "success" else "s")


def run_doctests(module, verbose=False):
    """Run doctests found in *module* and return the number of failures."""
    _failed = 0
    _errors = 0
    _total = 0
    for _name in sorted(dir(module)):
        _obj = getattr(module, _name)
        if not callable(_obj):
            continue
        _doc = getattr(_obj, "__doc__", None)
        if not _doc or ">>>" not in _doc:
            continue
        for _src, _expected in _extract_examples(_doc):
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
                    _sys.stderr.write(
                        "Error in example:\n"
                        "    " + _src.replace("\n", "\n    ") + "\n"
                        "Got:\n"
                        "    " + _got.replace("\n", "\n    ") + "\n"
                    )
                else:
                    _failed += 1
                    _sys.stderr.write(
                        "Failed example:\n"
                        "    " + _src.replace("\n", "\n    ") + "\n"
                        "Expected:\n"
                        "    " + _expected.replace("\n", "\n    ") + "\n"
                        "Got:\n"
                        "    " + _got.replace("\n", "\n    ") + "\n"
                    )
            elif verbose:
                _sys.stderr.write("ok: " + _src.split("\n")[0] + "\n")
    if _total > 0:
        _successes = _total - _failed - _errors
        _sys.stderr.write(
            "Running tests...\n"
            + _plural(_total, "test")
            + ", "
            + _plural(_successes, "success")
            + ", "
            + _plural(_failed, "failure")
            + " and "
            + _plural(_errors, "error")
            + ".\n"
        )
    return _failed + _errors
