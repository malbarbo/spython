//! The prelude, the Python that the compiler compiles along with a program.
//! The checker reads it as a package, and no program may import it.

/// The name of the package of the prelude.
pub const PACKAGE: &str = "__spython_prelude__";

/// Each file of the package, with its text.
pub const FILES: &[(&str, &str)] = &[
    ("__init__.py", include_str!("../python/__init__.py")),
    ("consumers.py", include_str!("../python/consumers.py")),
    ("dict.py", include_str!("../python/dict.py")),
    ("doctest.py", include_str!("../python/doctest.py")),
    ("float.py", include_str!("../python/float.py")),
    ("int.py", include_str!("../python/int.py")),
    ("intrinsics.pyi", include_str!("../python/intrinsics.pyi")),
    ("list.py", include_str!("../python/list.py")),
    ("math.py", include_str!("../python/math.py")),
    ("set.py", include_str!("../python/set.py")),
    ("slices.py", include_str!("../python/slices.py")),
    ("str.py", include_str!("../python/str.py")),
];
