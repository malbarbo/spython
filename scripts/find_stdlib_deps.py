#!/usr/bin/env python3
"""Find the transitive stdlib dependencies of a set of seed modules.

Usage:
    python3 scripts/find_stdlib_deps.py crates/RustPython/Lib dataclasses encodings

Traces imports at the **file level** within packages so that only the
actually-needed submodules are included.  For example, ``inspect`` imports
``importlib.machinery`` but NOT ``importlib.metadata``, so the latter (and
its heavy ``email`` dependency chain) is excluded.

Output: sorted fully-qualified module names, one per line.
"""

import ast
import os
import sys


def get_imports(filepath, package=None):
    """Return dotted module names imported by *filepath*.

    *package* is the dotted package name that *filepath* belongs to (used to
    resolve relative imports).  Pass ``None`` for top-level modules.

    Only considers imports at module level — skips imports inside functions,
    try/except handlers, and ``if __name__ == "__main__"`` guards.
    """
    try:
        with open(filepath) as f:
            tree = ast.parse(f.read(), filename=filepath)
    except (SyntaxError, UnicodeDecodeError):
        return set()

    raw_imports = set()
    for node in ast.iter_child_nodes(tree):
        _collect_imports(node, raw_imports, package)
    return raw_imports


def _is_main_guard(node):
    if not isinstance(node, ast.If):
        return False
    test = node.test
    if isinstance(test, ast.Compare) and len(test.ops) == 1:
        if isinstance(test.ops[0], ast.Eq):
            left = test.left
            comps = test.comparators
            if (
                isinstance(left, ast.Name)
                and left.id == "__name__"
                and len(comps) == 1
                and isinstance(comps[0], ast.Constant)
                and comps[0].value == "__main__"
            ):
                return True
    return False


def _resolve_relative(module, level, package):
    """Resolve a relative import to an absolute dotted name."""
    if package is None or level == 0:
        return module
    parts = package.split(".")
    # level=1 means current package, level=2 means parent, etc.
    if level > len(parts):
        return module  # can't resolve, return as-is
    base = ".".join(parts[: len(parts) - level + 1])
    if module:
        return base + "." + module
    return base


def _collect_imports(node, imports, package=None):
    if isinstance(node, ast.Import):
        for alias in node.names:
            imports.add(alias.name)
    elif isinstance(node, ast.ImportFrom):
        if node.level > 0:
            # Relative import
            resolved = _resolve_relative(node.module or "", node.level, package)
            imports.add(resolved)
            if node.names:
                for alias in node.names:
                    if alias.name != "*":
                        imports.add(resolved + "." + alias.name)
        elif node.module:
            imports.add(node.module)
            if node.names:
                for alias in node.names:
                    if alias.name != "*":
                        imports.add(node.module + "." + alias.name)
    elif isinstance(node, (ast.If, ast.While, ast.For)):
        if _is_main_guard(node):
            return
        for child in node.body:
            _collect_imports(child, imports, package)
        if hasattr(node, "orelse"):
            for child in node.orelse:
                _collect_imports(child, imports, package)
    elif isinstance(node, ast.Try):
        for child in node.body:
            _collect_imports(child, imports, package)
    elif isinstance(node, ast.ClassDef):
        for child in node.body:
            _collect_imports(child, imports, package)


def resolve_module(dotted_name, lib_dir):
    """Resolve a dotted module name to a file path, or None if built-in.

    Returns (filepath, is_package).
    """
    parts = dotted_name.split(".")
    # Try as a package: foo/bar/__init__.py
    pkg_path = os.path.join(lib_dir, *parts, "__init__.py")
    if os.path.isfile(pkg_path):
        return pkg_path, True
    # Try as a module: foo/bar.py
    mod_path = os.path.join(lib_dir, *parts) + ".py"
    if os.path.isfile(mod_path):
        return mod_path, False
    return None, False


def collect_deps(seeds, lib_dir):
    """Return all dotted module names transitively imported by *seeds*."""
    seen_modules = set()  # dotted names we've processed
    seen_files = set()  # files we've read imports from
    stack = list(seeds)

    while stack:
        dotted = stack.pop()
        if dotted in seen_modules:
            continue
        seen_modules.add(dotted)

        result, is_package = resolve_module(dotted, lib_dir)
        if result is None:
            continue

        # For a package, also ensure parent packages are included
        parts = dotted.split(".")
        for i in range(1, len(parts)):
            parent = ".".join(parts[:i])
            if parent not in seen_modules:
                stack.append(parent)

        if result in seen_files:
            continue
        seen_files.add(result)

        # Determine the package context for resolving relative imports
        if is_package:
            pkg_context = dotted
        elif "." in dotted:
            pkg_context = dotted.rsplit(".", 1)[0]
        else:
            pkg_context = None

        for imp in get_imports(result, package=pkg_context):
            if imp not in seen_modules:
                stack.append(imp)

    return seen_modules


def main():
    if len(sys.argv) < 3:
        print(
            f"Usage: {sys.argv[0]} <lib_dir> <module> [<module> ...]",
            file=sys.stderr,
        )
        sys.exit(1)

    lib_dir = sys.argv[1]
    seeds = sys.argv[2:]

    all_deps = collect_deps(seeds, lib_dir)

    # The encodings package loads codecs dynamically by name at runtime.
    # Always include the essential ones needed by the VM init.
    if "encodings" in all_deps:
        all_deps.update([
            "encodings.aliases",
            "encodings.ascii",
            "encodings.latin_1",
            "encodings.utf_8",
        ])

    # Keep only modules that resolve to actual files in lib_dir
    resolved = sorted(
        m for m in all_deps if resolve_module(m, lib_dir)[0] is not None
    )

    for m in resolved:
        print(m)

    # Summary on stderr
    total_bytes = 0
    files_seen = set()
    for m in resolved:
        path, _ = resolve_module(m, lib_dir)
        if path and path not in files_seen:
            files_seen.add(path)
            total_bytes += os.path.getsize(path)
    print(
        f"\n# {len(resolved)} modules, {len(files_seen)} files, "
        f"{total_bytes:,} bytes ({total_bytes/1024:.0f} KB)",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
