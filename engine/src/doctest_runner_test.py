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
    assert _parse(doc, "f") == [("def f():\n    return 1\n", ""), ("f()\n", "1\n")]


def test_parse_keeps_the_indentation_of_the_output():
    doc = '    >>> print("  x")\n      x\n'
    assert _parse(doc, "f") == [('print("  x")\n', "  x\n")]


def test_parse_removes_the_margin_of_the_docstring():
    assert _parse("\n    text\n\n    >>> 1\n    1\n    ", "f") == [("1\n", "1\n")]


def test_parse_ends_the_output_at_a_blank_line():
    doc = '>>> print("a")\na\n\nb\n'
    assert _parse(doc, "f") == [('print("a")\n', "a\n")]


def test_parse_skips_an_example_that_is_only_a_comment():
    assert _parse(">>> # nothing\n>>> 2\n2\n", "f") == [("2\n", "2\n")]


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


for _name, _test in list(globals().items()):
    if _name.startswith("test_"):
        _test()
