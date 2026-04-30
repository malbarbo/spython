//! Embedded copy of `lib/spython/*.py` so ty's resolver can see the spython
//! package at type-check time. The frozen RustPython bytecode handles
//! runtime; these embedded sources are only consumed by the type checker.
//!
//! The lib is written to a `_spython_lib/spython/` subtree under whatever
//! root the caller provides, then that root is added to ty's `extra-paths`.

use ruff_db::system::{SystemPath, SystemPathBuf, WritableSystem};

/// Name of the directory we add to `extra-paths`. Its sole child is `spython/`.
pub const LIB_DIR: &str = "_spython_lib";

/// Files that make up `lib/spython/`, embedded at compile time.
pub const FILES: &[(&str, &str)] = &[
    ("__init__.py", include_str!("../../lib/spython/__init__.py")),
    ("color.py", include_str!("../../lib/spython/color.py")),
    ("font.py", include_str!("../../lib/spython/font.py")),
    ("image.py", include_str!("../../lib/spython/image.py")),
    ("style.py", include_str!("../../lib/spython/style.py")),
    ("system.py", include_str!("../../lib/spython/system.py")),
    ("world.py", include_str!("../../lib/spython/world.py")),
];

/// Write the embedded spython lib into `system` under `root/_spython_lib/spython/`.
/// Returns the path to add to `extra-paths` (i.e. `root/_spython_lib`).
pub fn write_into<S: WritableSystem>(system: &S, root: &SystemPath) -> SystemPathBuf {
    let lib_root = root.join(LIB_DIR);
    let pkg_dir = lib_root.join("spython");
    let _ = system.create_directory_all(&pkg_dir);
    for (name, content) in FILES {
        let _ = system.write_file(&pkg_dir.join(name), content);
    }
    lib_root
}

/// Returns true if `path` points inside the embedded spython lib.
pub fn is_lib_path(path: &SystemPath) -> bool {
    let s = path.as_str().replace('\\', "/");
    s.contains(&format!("/{LIB_DIR}/spython/"))
}
