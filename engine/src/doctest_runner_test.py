# Tests of the doctest runner, which run after the runner in one script. The
# answers are what the stdlib `doctest` gives for the same docstrings.


def _parse_error(doc):
    try:
        _parse(doc, "f")
    except ValueError as e:
        return str(e)
    raise AssertionError("expected a ValueError for %r" % doc)


def test_parse_takes_a_bare_continuation_as_a_line_of_source():
    doc = ">>> def f():\n...     return 1\n...\n>>> f()\n1\n"
    assert _parse(doc, "f") == [("def f():\n    return 1\n", "", None), ("f()\n", "1\n", None)]


def test_parse_keeps_the_indentation_of_the_output():
    doc = '    >>> print("  x")\n      x\n'
    assert _parse(doc, "f") == [('print("  x")\n', "  x\n", None)]


def test_parse_removes_the_margin_of_the_docstring():
    assert _parse("\n    text\n\n    >>> 1\n    1\n    ", "f") == [("1\n", "1\n", None)]


def test_parse_ends_the_output_at_a_blank_line():
    doc = '>>> print("a")\na\n\nb\n'
    assert _parse(doc, "f") == [('print("a")\n', "a\n", None)]


def test_parse_skips_an_example_that_is_only_a_comment():
    assert _parse(">>> # nothing\n>>> 2\n2\n", "f") == [("2\n", "2\n", None)]


def test_parse_reads_the_exception_after_a_traceback_header():
    doc = ">>> f()\nTraceback (most recent call last):\n  ...\nValueError: m\n"
    want = "Traceback (most recent call last):\n  ...\nValueError: m\n"
    assert _parse(doc, "f") == [("f()\n", want, "ValueError: m\n")]


def test_parse_refuses_a_prompt_with_no_space():
    assert _parse_error(">>>x\n") == (
        "line 1 of the docstring for f lacks blank after >>>: '>>>x'"
    )
    assert _parse_error(">>> f(\n...1)\n") == (
        "line 2 of the docstring for f lacks blank after ...: '...1)'"
    )


def test_parse_refuses_a_line_that_does_not_line_up():
    assert _parse_error("  >>> f()\n... 1\n") == (
        "line 1 of the docstring for f has inconsistent leading whitespace: '... 1'"
    )
    assert _parse_error("  >>> f()\n 1\n") == (
        "line 2 of the docstring for f has inconsistent leading whitespace: '1'"
    )


def test_check_output_takes_blankline_for_a_blank_line():
    assert _check_output("a\n<BLANKLINE>\nb\n", "a\n\nb\n")
    assert _check_output("a\n<BLANKLINE>  \n", "a\n   \n")


def test_check_output_tells_a_missing_newline():
    assert not _check_output("a\n", "a\n\n")
    assert not _check_output("a\n", "a")


def test_check_output_does_not_take_true_for_1():
    assert not _check_output("1\n", "True\n")


def test_shown_marks_a_blank_line():
    assert _shown("a\n\n") == "a\n<BLANKLINE>"
    assert _shown("a\n  \nb\n") == "a\n<BLANKLINE>\nb"


def _run(source):
    """The number of examples of *source* that did not pass, and what the
    runner wrote to stderr."""
    import types

    module = types.ModuleType("m")
    exec(source, vars(module))
    old = _sys.stderr
    _sys.stderr = err = _Capture()
    try:
        failures = run_doctests(module)
    finally:
        _sys.stderr = old
    return failures, err.getvalue()


def test_run_counts_a_docstring_it_cannot_read_as_an_error():
    failures, err = _run('def f():\n    """\n    >>>x\n    """\n')
    assert failures == 1
    assert err == (
        "Running tests...\n"
        "  f\n"
        "    line 2 of the docstring for f lacks blank after >>>: '>>>x'\n"
        "1 test, 0 successes, 0 failures and 1 error.\n"
    )


def test_run_shows_the_repr_of_a_value_as_the_prompt_does():
    failures, err = _run(
        'def f() -> str:\n    """\n    >>> x = f(); x\n    \'a\'\n    """\n    return "a"\n'
    )
    assert failures == 0, err


def test_run_runs_an_example_once():
    source = (
        "calls = []\n"
        "def f() -> None:\n"
        '    """\n'
        "    >>> f()\n"
        "    SyntaxError: bad\n"
        "    >>> len(calls)\n"
        "    1\n"
        '    """\n'
        "    calls.append(1)\n"
        "    raise SyntaxError('bad')\n"
    )
    failures, err = _run(source)
    assert failures == 0, err


def test_run_takes_an_exception_after_a_traceback_header():
    source = (
        "def f() -> None:\n"
        '    """\n'
        "    >>> f()\n"
        "    Traceback (most recent call last):\n"
        "      ...\n"
        "    ValueError: m\n"
        '    """\n'
        "    print('ignored')\n"
        "    raise ValueError('m')\n"
    )
    failures, err = _run(source)
    assert failures == 0, err


def test_run_takes_an_exception_as_the_last_line_of_output():
    source = (
        "def f() -> None:\n"
        '    """\n'
        "    >>> f()\n"
        "    printed\n"
        "    ValueError\n"
        '    """\n'
        "    print('printed')\n"
        "    raise ValueError()\n"
    )
    failures, err = _run(source)
    assert failures == 0, err


def test_run_fails_an_example_that_raises_another_exception():
    source = (
        "def f() -> None:\n"
        '    """\n'
        "    >>> f()\n"
        "    Traceback (most recent call last):\n"
        "    KeyError: 'k'\n"
        '    """\n'
        "    raise ValueError('m')\n"
    )
    failures, err = _run(source)
    assert failures == 1
    assert "    Got:\n      ValueError: m\n" in err, err


def test_run_refuses_two_statements_in_one_example():
    failures, err = _run('def f() -> None:\n    """\n    >>> x = 1\n    ... y = 2\n    """\n')
    assert failures == 1
    assert "SyntaxError: multiple statements found while compiling a single statement" in err


def test_run_stops_at_an_interrupt():
    source = (
        "def f() -> None:\n"
        '    """\n'
        "    >>> f()\n"
        "    KeyboardInterrupt\n"
        '    """\n'
        "    raise KeyboardInterrupt\n"
    )
    old = _sys.stdout
    try:
        _run(source)
    except KeyboardInterrupt:
        assert _sys.stdout is old
    else:
        raise AssertionError("expected the interrupt to stop the run")


def test_exception_line_names_the_module_of_an_imported_class():
    import types

    module = types.ModuleType("m")

    class E(Exception):
        pass

    E.__module__ = "helpers"
    assert _exception_line(E("x"), module) == "helpers.test_exception_line_names_the_module_of_an_imported_class.<locals>.E: x\n"
    E.__module__ = "m"
    assert _exception_line(E(), module) == "test_exception_line_names_the_module_of_an_imported_class.<locals>.E\n"


def test_exception_line_ends_with_the_notes():
    import types

    e = ValueError("m")
    e.add_note("a note")
    assert _exception_line(e, types.ModuleType("m")) == "ValueError: m\na note\n"


for _name, _test in list(globals().items()):
    if _name.startswith("test_"):
        _test()
