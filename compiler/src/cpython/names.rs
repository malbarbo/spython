//! Names that CPython 3.13 knows, as python3.13 lists them.

/// The globals that a module of CPython has before its code runs, in order.
#[rustfmt::skip]
pub const MODULE_GLOBALS: &[&str] = &[
    "__name__", "__doc__", "__package__", "__loader__", "__spec__", "__annotations__",
    "__builtins__", "__file__", "__cached__",
];

/// The builtins of CPython 3.13 with the `site` module, in order.
#[rustfmt::skip]
pub const BUILTINS: &[&str] = &[
    "__name__", "__doc__", "__package__", "__loader__", "__spec__", "__build_class__",
    "__import__", "abs", "all", "any", "ascii", "bin", "breakpoint", "callable", "chr", "compile",
    "delattr", "dir", "divmod", "eval", "exec", "format", "getattr", "globals", "hasattr", "hash",
    "hex", "id", "input", "isinstance", "issubclass", "iter", "aiter", "len", "locals", "max",
    "min", "next", "anext", "oct", "ord", "pow", "print", "repr", "round", "setattr", "sorted",
    "sum", "vars", "None", "Ellipsis", "NotImplemented", "False", "True", "bool", "memoryview",
    "bytearray", "bytes", "classmethod", "complex", "dict", "enumerate", "filter", "float",
    "frozenset", "property", "int", "list", "map", "object", "range", "reversed", "set", "slice",
    "staticmethod", "str", "super", "tuple", "type", "zip", "__debug__", "BaseException",
    "BaseExceptionGroup", "Exception", "GeneratorExit", "KeyboardInterrupt", "SystemExit",
    "ArithmeticError", "AssertionError", "AttributeError", "BufferError", "EOFError",
    "ImportError", "LookupError", "MemoryError", "NameError", "OSError", "ReferenceError",
    "RuntimeError", "StopAsyncIteration", "StopIteration", "SyntaxError", "SystemError",
    "TypeError", "ValueError", "Warning", "FloatingPointError", "OverflowError",
    "ZeroDivisionError", "BytesWarning", "DeprecationWarning", "EncodingWarning", "FutureWarning",
    "ImportWarning", "PendingDeprecationWarning", "ResourceWarning", "RuntimeWarning",
    "SyntaxWarning", "UnicodeWarning", "UserWarning", "BlockingIOError", "ChildProcessError",
    "ConnectionError", "FileExistsError", "FileNotFoundError", "InterruptedError",
    "IsADirectoryError", "NotADirectoryError", "PermissionError", "ProcessLookupError",
    "TimeoutError", "IndentationError", "_IncompleteInputError", "IndexError", "KeyError",
    "ModuleNotFoundError", "NotImplementedError", "PythonFinalizationError", "RecursionError",
    "UnboundLocalError", "UnicodeError", "BrokenPipeError", "ConnectionAbortedError",
    "ConnectionRefusedError", "ConnectionResetError", "TabError", "UnicodeDecodeError",
    "UnicodeEncodeError", "UnicodeTranslateError", "ExceptionGroup", "EnvironmentError", "IOError",
    "open", "quit", "exit", "copyright", "credits", "license", "help",
];

/// The attributes of an `int`, sorted for a binary search.
#[rustfmt::skip]
pub const INT_ATTRIBUTES: &[&str] = &[
    "__abs__", "__add__", "__and__", "__bool__", "__ceil__", "__class__", "__delattr__", "__dir__",
    "__divmod__", "__doc__", "__eq__", "__float__", "__floor__", "__floordiv__", "__format__",
    "__ge__", "__getattribute__", "__getnewargs__", "__getstate__", "__gt__", "__hash__",
    "__index__", "__init__", "__init_subclass__", "__int__", "__invert__", "__le__", "__lshift__",
    "__lt__", "__mod__", "__mul__", "__ne__", "__neg__", "__new__", "__or__", "__pos__", "__pow__",
    "__radd__", "__rand__", "__rdivmod__", "__reduce__", "__reduce_ex__", "__repr__",
    "__rfloordiv__", "__rlshift__", "__rmod__", "__rmul__", "__ror__", "__round__", "__rpow__",
    "__rrshift__", "__rshift__", "__rsub__", "__rtruediv__", "__rxor__", "__setattr__",
    "__sizeof__", "__str__", "__sub__", "__subclasshook__", "__truediv__", "__trunc__", "__xor__",
    "as_integer_ratio", "bit_count", "bit_length", "conjugate", "denominator", "from_bytes",
    "imag", "is_integer", "numerator", "real", "to_bytes",
];

/// The attributes of a `bool`, sorted for a binary search.
#[rustfmt::skip]
pub const BOOL_ATTRIBUTES: &[&str] = &[
    "__abs__", "__add__", "__and__", "__bool__", "__ceil__", "__class__", "__delattr__", "__dir__",
    "__divmod__", "__doc__", "__eq__", "__float__", "__floor__", "__floordiv__", "__format__",
    "__ge__", "__getattribute__", "__getnewargs__", "__getstate__", "__gt__", "__hash__",
    "__index__", "__init__", "__init_subclass__", "__int__", "__invert__", "__le__", "__lshift__",
    "__lt__", "__mod__", "__mul__", "__ne__", "__neg__", "__new__", "__or__", "__pos__", "__pow__",
    "__radd__", "__rand__", "__rdivmod__", "__reduce__", "__reduce_ex__", "__repr__",
    "__rfloordiv__", "__rlshift__", "__rmod__", "__rmul__", "__ror__", "__round__", "__rpow__",
    "__rrshift__", "__rshift__", "__rsub__", "__rtruediv__", "__rxor__", "__setattr__",
    "__sizeof__", "__str__", "__sub__", "__subclasshook__", "__truediv__", "__trunc__", "__xor__",
    "as_integer_ratio", "bit_count", "bit_length", "conjugate", "denominator", "from_bytes",
    "imag", "is_integer", "numerator", "real", "to_bytes",
];

/// The attributes of a `str`, sorted for a binary search.
#[rustfmt::skip]
pub const STR_ATTRIBUTES: &[&str] = &[
    "__add__", "__class__", "__contains__", "__delattr__", "__dir__", "__doc__", "__eq__",
    "__format__", "__ge__", "__getattribute__", "__getitem__", "__getnewargs__", "__getstate__",
    "__gt__", "__hash__", "__init__", "__init_subclass__", "__iter__", "__le__", "__len__",
    "__lt__", "__mod__", "__mul__", "__ne__", "__new__", "__reduce__", "__reduce_ex__", "__repr__",
    "__rmod__", "__rmul__", "__setattr__", "__sizeof__", "__str__", "__subclasshook__",
    "capitalize", "casefold", "center", "count", "encode", "endswith", "expandtabs", "find",
    "format", "format_map", "index", "isalnum", "isalpha", "isascii", "isdecimal", "isdigit",
    "isidentifier", "islower", "isnumeric", "isprintable", "isspace", "istitle", "isupper", "join",
    "ljust", "lower", "lstrip", "maketrans", "partition", "removeprefix", "removesuffix",
    "replace", "rfind", "rindex", "rjust", "rpartition", "rsplit", "rstrip", "split", "splitlines",
    "startswith", "strip", "swapcase", "title", "translate", "upper", "zfill",
];

/// The attributes of a function, sorted for a binary search.
#[rustfmt::skip]
pub const FUNCTION_ATTRIBUTES: &[&str] = &[
    "__annotations__", "__builtins__", "__call__", "__class__", "__closure__", "__code__",
    "__defaults__", "__delattr__", "__dict__", "__dir__", "__doc__", "__eq__", "__format__",
    "__ge__", "__get__", "__getattribute__", "__getstate__", "__globals__", "__gt__", "__hash__",
    "__init__", "__init_subclass__", "__kwdefaults__", "__le__", "__lt__", "__module__",
    "__name__", "__ne__", "__new__", "__qualname__", "__reduce__", "__reduce_ex__", "__repr__",
    "__setattr__", "__sizeof__", "__str__", "__subclasshook__", "__type_params__",
];

/// The modules of the standard library of CPython 3.13, sorted for a
/// binary search.
#[rustfmt::skip]
pub const STDLIB_MODULES: &[&str] = &[
    "__future__", "_abc", "_aix_support", "_android_support", "_apple_support", "_ast", "_asyncio",
    "_bisect", "_blake2", "_bz2", "_codecs", "_codecs_cn", "_codecs_hk", "_codecs_iso2022",
    "_codecs_jp", "_codecs_kr", "_codecs_tw", "_collections", "_collections_abc", "_colorize",
    "_compat_pickle", "_compression", "_contextvars", "_csv", "_ctypes", "_curses",
    "_curses_panel", "_datetime", "_dbm", "_decimal", "_elementtree", "_frozen_importlib",
    "_frozen_importlib_external", "_functools", "_gdbm", "_hashlib", "_heapq", "_imp",
    "_interpchannels", "_interpqueues", "_interpreters", "_io", "_ios_support", "_json", "_locale",
    "_lsprof", "_lzma", "_markupbase", "_md5", "_multibytecodec", "_multiprocessing", "_opcode",
    "_opcode_metadata", "_operator", "_osx_support", "_overlapped", "_pickle", "_posixshmem",
    "_posixsubprocess", "_py_abc", "_pydatetime", "_pydecimal", "_pyio", "_pylong", "_pyrepl",
    "_queue", "_random", "_scproxy", "_sha1", "_sha2", "_sha3", "_signal", "_sitebuiltins",
    "_socket", "_sqlite3", "_sre", "_ssl", "_stat", "_statistics", "_string", "_strptime",
    "_struct", "_suggestions", "_symtable", "_sysconfig", "_thread", "_threading_local",
    "_tkinter", "_tokenize", "_tracemalloc", "_typing", "_uuid", "_warnings", "_weakref",
    "_weakrefset", "_winapi", "_wmi", "_zoneinfo", "abc", "antigravity", "argparse", "array",
    "ast", "asyncio", "atexit", "base64", "bdb", "binascii", "bisect", "builtins", "bz2",
    "cProfile", "calendar", "cmath", "cmd", "code", "codecs", "codeop", "collections", "colorsys",
    "compileall", "concurrent", "configparser", "contextlib", "contextvars", "copy", "copyreg",
    "csv", "ctypes", "curses", "dataclasses", "datetime", "dbm", "decimal", "difflib", "dis",
    "doctest", "email", "encodings", "ensurepip", "enum", "errno", "faulthandler", "fcntl",
    "filecmp", "fileinput", "fnmatch", "fractions", "ftplib", "functools", "gc", "genericpath",
    "getopt", "getpass", "gettext", "glob", "graphlib", "grp", "gzip", "hashlib", "heapq", "hmac",
    "html", "http", "idlelib", "imaplib", "importlib", "inspect", "io", "ipaddress", "itertools",
    "json", "keyword", "linecache", "locale", "logging", "lzma", "mailbox", "marshal", "math",
    "mimetypes", "mmap", "modulefinder", "msvcrt", "multiprocessing", "netrc", "nt", "ntpath",
    "nturl2path", "numbers", "opcode", "operator", "optparse", "os", "pathlib", "pdb", "pickle",
    "pickletools", "pkgutil", "platform", "plistlib", "poplib", "posix", "posixpath", "pprint",
    "profile", "pstats", "pty", "pwd", "py_compile", "pyclbr", "pydoc", "pydoc_data", "pyexpat",
    "queue", "quopri", "random", "re", "readline", "reprlib", "resource", "rlcompleter", "runpy",
    "sched", "secrets", "select", "selectors", "shelve", "shlex", "shutil", "signal", "site",
    "smtplib", "socket", "socketserver", "sqlite3", "sre_compile", "sre_constants", "sre_parse",
    "ssl", "stat", "statistics", "string", "stringprep", "struct", "subprocess", "symtable", "sys",
    "sysconfig", "syslog", "tabnanny", "tarfile", "tempfile", "termios", "textwrap", "this",
    "threading", "time", "timeit", "tkinter", "token", "tokenize", "tomllib", "trace", "traceback",
    "tracemalloc", "tty", "turtle", "turtledemo", "types", "typing", "unicodedata", "unittest",
    "urllib", "uuid", "venv", "warnings", "wave", "weakref", "webbrowser", "winreg", "winsound",
    "wsgiref", "xml", "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
];
