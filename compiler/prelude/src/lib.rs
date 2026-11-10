//! The prelude, the Python that the compiler compiles along with a program.
//! The checker reads it as a package, and no program may import it.

/// The name of the package of the prelude.
pub const PACKAGE: &str = "__spython_prelude__";

/// Each file of the package, with its text.
pub const FILES: &[(&str, &str)] = &[
    ("__init__.py", include_str!("../python/__init__.py")),
    ("intrinsics.pyi", include_str!("../python/intrinsics.pyi")),
    ("math.py", include_str!("../python/math.py")),
];
