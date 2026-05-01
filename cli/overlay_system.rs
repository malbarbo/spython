//! `OverlaySystem`: an `OsSystem` with the embedded spython lib mounted at a
//! synthetic path. ty's resolver only sees `System` method calls, so we can
//! serve the lib's source files from memory without ever touching disk.
//!
//! Why: the lib is embedded into the binary via `include_str!`, so writing
//! it to a real cache directory just so ty's `OsSystem`-backed resolver can
//! find it is wasted I/O — and a shared cache directory races between
//! concurrent `spython` subprocesses (e.g. parallel `cargo test` workers
//! on Windows CI), causing one process's resolver to see a missing or
//! half-written `spython/` package.

use ruff_db::system::walk_directory::WalkDirectoryBuilder;
use ruff_db::system::{
    CaseSensitivity, DirectoryEntry, GlobError, MemoryFileSystem, Metadata, OsSystem, PatternError,
    Result, System, SystemPath, SystemPathBuf, SystemVirtualPath, WhichResult, WritableSystem,
};
use ruff_notebook::{Notebook, NotebookError};
use std::any::Any;
use std::sync::Arc;

/// The synthetic mount point under which the embedded spython lib lives.
/// Chosen so it can never collide with a real user path: on every supported
/// host, a path under `__spython_embedded__` (relative to the OS cache dir)
/// is well-formed but never created on disk.
fn synthetic_lib_root() -> SystemPathBuf {
    let cache_root = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    let p = cache_root.join("__spython_embedded__");
    SystemPathBuf::from_path_buf(p)
        .unwrap_or_else(|_| SystemPathBuf::from("/__spython_embedded__"))
        .join(engine::spython_lib::LIB_DIR)
}

#[derive(Debug, Clone)]
pub struct OverlaySystem {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    os: OsSystem,
    mem: MemoryFileSystem,
    lib_root: SystemPathBuf,
}

impl OverlaySystem {
    /// Build an overlay rooted at `cwd` with the embedded spython lib mounted
    /// at a synthetic path. Returns `(system, lib_root)`; pass `lib_root` to
    /// ty's `extra-paths`.
    pub fn new(cwd: &SystemPath) -> (Self, SystemPathBuf) {
        let os = OsSystem::new(cwd);
        let lib_root = synthetic_lib_root();
        let pkg_dir = lib_root.join("spython");
        let mem = MemoryFileSystem::new();
        // Errors writing the in-memory FS shouldn't happen, but if they
        // somehow do the lib just won't resolve — type errors will surface
        // at the call site as unresolved-import, which is louder than a
        // silent miscompile.
        let _ = mem.create_directory_all(&pkg_dir);
        for (name, content) in engine::spython_lib::FILES {
            let _ = mem.write_file(pkg_dir.join(name), content);
        }
        let system = Self {
            inner: Arc::new(Inner {
                os,
                mem,
                lib_root: lib_root.clone(),
            }),
        };
        (system, lib_root)
    }

    fn is_lib(&self, path: &SystemPath) -> bool {
        path.starts_with(&self.inner.lib_root)
    }
}

impl System for OverlaySystem {
    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {
        if self.is_lib(path) {
            self.inner.mem.metadata(path)
        } else {
            self.inner.os.path_metadata(path)
        }
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        if self.is_lib(path) {
            self.inner.mem.canonicalize(path)
        } else {
            self.inner.os.canonicalize_path(path)
        }
    }

    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        if self.is_lib(path) {
            self.inner.mem.read_to_string(path)
        } else {
            self.inner.os.read_to_string(path)
        }
    }

    fn read_to_notebook(&self, path: &SystemPath) -> std::result::Result<Notebook, NotebookError> {
        if self.is_lib(path) {
            Notebook::from_source_code(&self.inner.mem.read_to_string(path)?)
        } else {
            self.inner.os.read_to_notebook(path)
        }
    }

    fn read_virtual_path_to_string(&self, path: &SystemVirtualPath) -> Result<String> {
        self.inner.os.read_virtual_path_to_string(path)
    }

    fn read_virtual_path_to_notebook(
        &self,
        path: &SystemVirtualPath,
    ) -> std::result::Result<Notebook, NotebookError> {
        self.inner.os.read_virtual_path_to_notebook(path)
    }

    fn path_exists_case_sensitive(&self, path: &SystemPath, prefix: &SystemPath) -> bool {
        if self.is_lib(path) {
            // The in-memory lib is case-sensitive and we control the casing.
            self.inner.mem.exists(path)
        } else {
            self.inner.os.path_exists_case_sensitive(path, prefix)
        }
    }

    fn case_sensitivity(&self) -> CaseSensitivity {
        self.inner.os.case_sensitivity()
    }

    fn which(&self, name: &str) -> WhichResult {
        self.inner.os.which(name)
    }

    fn current_directory(&self) -> &SystemPath {
        self.inner.os.current_directory()
    }

    fn user_config_directory(&self) -> Option<SystemPathBuf> {
        self.inner.os.user_config_directory()
    }

    fn cache_dir(&self) -> Option<SystemPathBuf> {
        self.inner.os.cache_dir()
    }

    fn read_directory<'a>(
        &'a self,
        path: &SystemPath,
    ) -> Result<Box<dyn Iterator<Item = Result<DirectoryEntry>> + 'a>> {
        if self.is_lib(path) {
            Ok(Box::new(self.inner.mem.read_directory(path)?))
        } else {
            self.inner.os.read_directory(path)
        }
    }

    fn walk_directory(&self, path: &SystemPath) -> WalkDirectoryBuilder {
        if self.is_lib(path) {
            self.inner.mem.walk_directory(path)
        } else {
            self.inner.os.walk_directory(path)
        }
    }

    fn glob(
        &self,
        pattern: &str,
    ) -> std::result::Result<
        Box<dyn Iterator<Item = std::result::Result<SystemPathBuf, GlobError>> + '_>,
        PatternError,
    > {
        // Globs aren't used to resolve the lib (ty walks it directly via
        // search paths), so route everything through the OS impl.
        self.inner.os.glob(pattern)
    }

    fn env_var(&self, name: &str) -> std::result::Result<String, std::env::VarError> {
        self.inner.os.env_var(name)
    }

    fn as_writable(&self) -> Option<&dyn WritableSystem> {
        // The embedded lib is read-only and we don't want callers writing
        // through the overlay anyway — they'd bypass our routing.
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn dyn_clone(&self) -> Box<dyn System> {
        Box::new(self.clone())
    }
}
