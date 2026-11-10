//! The texts of the module `dataclasses` of CPython 3.13 on Linux.

/// The frames of `dataclasses.py` under the decorator of a class, when the
/// field of a default that is an object of a dataclass raises `ValueError`.
/// The decorator without arguments calls `wrap` from `dataclass`.
pub(crate) fn mutable_default_frames(call: bool) -> String {
    let dataclass = if call {
        ""
    } else {
        concat!(
            "  File \"/usr/lib/python3.13/dataclasses.py\", line 1305, in dataclass\n",
            "    return wrap(cls)\n",
        )
    };
    format!(
        "{dataclass}{}",
        concat!(
            "  File \"/usr/lib/python3.13/dataclasses.py\", line 1295, in wrap\n",
            "    return _process_class(cls, init, repr, eq, order, unsafe_hash,\n",
            "                          frozen, match_args, kw_only, slots,\n",
            "                          weakref_slot)\n",
            "  File \"/usr/lib/python3.13/dataclasses.py\", line 1008, in _process_class\n",
            "    cls_fields.append(_get_field(cls, name, type, kw_only))\n",
            "                      ~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^\n",
            "  File \"/usr/lib/python3.13/dataclasses.py\", line 860, in _get_field\n",
            "    raise ValueError(f'mutable default {type(f.default)} for field '\n",
            "                     f'{f.name} is not allowed: use default_factory')\n",
        )
    )
}

/// The text of the `ValueError` of the field `field` with a default that
/// is an object of the dataclass `class`, of the module `__main__`.
pub(crate) fn mutable_default(class: &str, field: &str) -> String {
    format!(
        "mutable default <class '__main__.{class}'> for field {field} is not allowed: use default_factory"
    )
}

/// The names that the namespace of a class has before its first
/// statement, and `__annotations__` and `__doc__` after them when the
/// class has an annotation and a docstring.
pub(crate) const CLASS_NAMESPACE: [&str; 3] = ["__module__", "__qualname__", "__firstlineno__"];
