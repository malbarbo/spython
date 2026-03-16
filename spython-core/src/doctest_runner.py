"""Minimal doctest runner that avoids the stdlib `doctest` module.

The stdlib module requires `inspect` → `dis` → `io` → `_io.FileIO`,
which is not available in the RustPython WASM build.
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


def _run_doctests():
    import __main__

    _failed = 0
    _total = 0
    for _name in sorted(dir(__main__)):
        _obj = getattr(__main__, _name)
        if not callable(_obj):
            continue
        _doc = getattr(_obj, "__doc__", None)
        if not _doc or ">>>" not in _doc:
            continue
        for _src, _expected in _extract_examples(_doc):
            _total += 1
            _old_stdout = _sys.stdout
            _sys.stdout = _buf = _Capture()
            try:
                _code = compile(_src, "<doctest>", "eval")
                _result = eval(_code, vars(__main__))
                if _result is not None:
                    print(repr(_result))
            except SyntaxError:
                _code = compile(_src, "<doctest>", "exec")
                exec(_code, vars(__main__))
            except BaseException as _e:
                print(type(_e).__name__ + ": " + str(_e))
            finally:
                _sys.stdout = _old_stdout
            _got = _buf.getvalue().rstrip("\n")
            if _got != _expected:
                _failed += 1
                _sys.stderr.write(
                    "Failed example:\n"
                    "    " + _src.replace("\n", "\n    ") + "\n"
                    "Expected:\n"
                    "    " + _expected.replace("\n", "\n    ") + "\n"
                    "Got:\n"
                    "    " + _got.replace("\n", "\n    ") + "\n"
                )
    if _failed:
        _sys.stderr.write(
            "***Test Failed*** "
            + str(_failed)
            + " of "
            + str(_total)
            + " example(s).\n"
        )


_run_doctests()
del _run_doctests, _extract_examples, _Capture, _sys
