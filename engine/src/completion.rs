use rustpython::{vm, vm::AsObject, vm::PyResult, vm::TryFromObject};
use vm::VirtualMachine;
use vm::builtins::{PyDictRef, PyStrRef};
use vm::function::ArgIterable;
use vm::identifier;

pub const PYTHON_BOOLEANS: &[&str] = &["False", "True"];
pub const PYTHON_CONSTANTS: &[&str] = &["None"];

pub const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "case", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "match", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with",
    "yield",
];

/// Result of pressing Tab in the REPL.
pub enum TabAction {
    /// Insert indentation spaces.
    Indent(String),
    /// Show completion candidates. `usize` is the start position for replacement.
    Complete(usize, Vec<String>),
    /// Do nothing.
    Nothing,
}

/// Decide whether Tab should indent or complete.
///
/// `text` is the full editor content, `cursor_pos` is the byte offset of the cursor.
pub fn tab_action(
    vm: &VirtualMachine,
    globals: &PyDictRef,
    text: &str,
    cursor_pos: usize,
) -> TabAction {
    let before = &text[..cursor_pos];
    let current_line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let current_line = &before[current_line_start..];

    if current_line.bytes().all(|b| b == b' ') {
        let current_indent = current_line.len();
        let expected = expected_indent(&before[..current_line_start]);
        if current_indent < expected {
            return TabAction::Indent(" ".repeat(expected - current_indent));
        }
        // At or above expected indent → show all completions.
        return match complete(vm, globals, "") {
            Some((_, candidates)) => TabAction::Complete(cursor_pos, candidates),
            None => TabAction::Nothing,
        };
    }

    match complete(vm, globals, before) {
        Some((startpos, candidates)) => TabAction::Complete(startpos, candidates),
        None => TabAction::Nothing,
    }
}

/// Complete identifiers at the end of `line`.
///
/// Returns `(startpos, candidates)` where `startpos` is the byte offset in `line`
/// where the matched prefix begins. Candidates are sorted and include globals,
/// builtins, attributes, and Python keywords.
pub fn complete(
    vm: &VirtualMachine,
    globals: &PyDictRef,
    line: &str,
) -> Option<(usize, Vec<String>)> {
    let (startpos, words) = if line.is_empty() {
        (0, vec![String::new()])
    } else {
        split_idents_on_dot(line)?
    };

    let (word_start, iter) = get_available_completions(vm, globals, &words)?;

    let all_completions: Vec<_> = iter
        .filter_map(|res| res.ok())
        .filter(|s| s.as_bytes().starts_with(word_start.as_bytes()))
        .collect();
    let completions = if word_start.starts_with('_') {
        all_completions
    } else {
        let no_underscore = all_completions
            .iter()
            .filter(|&s| !s.as_bytes().starts_with(b"_"))
            .cloned()
            .collect::<Vec<_>>();

        if no_underscore.is_empty() {
            all_completions
        } else {
            no_underscore
        }
    };

    let mut result: Vec<String> = completions
        .into_iter()
        .map(|s| s.expect_str().to_owned())
        .collect();

    // Add keyword completions for top-level (no dot).
    if words.len() == 1 {
        for kw in PYTHON_KEYWORDS
            .iter()
            .chain(PYTHON_BOOLEANS.iter())
            .chain(PYTHON_CONSTANTS.iter())
        {
            if kw.starts_with(word_start) && !result.iter().any(|s| s == kw) {
                result.push((*kw).to_owned());
            }
        }
    }

    result.sort();

    Some((startpos, result))
}

/// Compute the expected indentation level based on previous lines.
fn expected_indent(lines_before: &str) -> usize {
    let prev_line = lines_before.lines().rev().find(|l| !l.trim().is_empty());
    match prev_line {
        None => 0,
        Some(line) => {
            let indent = line.len() - line.trim_start().len();
            if line.trim_end().ends_with(':') {
                indent + 4
            } else {
                indent
            }
        }
    }
}

fn get_available_completions<'a>(
    vm: &'a VirtualMachine,
    globals: &PyDictRef,
    words: &'a [String],
) -> Option<(&'a str, impl Iterator<Item = PyResult<PyStrRef>> + 'a)> {
    let (first, rest) = words.split_first().unwrap();

    let str_iter_method = |obj, name| {
        let iter = vm.call_special_method(obj, name, ())?;
        ArgIterable::<PyStrRef>::try_from_object(vm, iter)?.iter(vm)
    };

    let (word_start, iter1, iter2) = if let Some((last, parents)) = rest.split_last() {
        let mut current = globals
            .get_item_opt(first.as_str(), vm)
            .ok()
            .flatten()
            .or_else(|| {
                let attr = vm.ctx.new_str(first.as_str());
                vm.builtins.get_attr(&attr, vm).ok()
            })?;

        for attr in parents {
            let attr = vm.ctx.new_str(attr.as_str());
            current = current.get_attr(&attr, vm).ok()?;
        }

        let current_iter = str_iter_method(&current, identifier!(vm, __dir__)).ok()?;

        (last, current_iter, None)
    } else {
        let g = str_iter_method(globals.as_object(), identifier!(vm, keys)).ok()?;
        let b = str_iter_method(vm.builtins.as_object(), identifier!(vm, __dir__)).ok()?;
        (first, g, Some(b))
    };
    Some((word_start, iter1.chain(iter2.into_iter().flatten())))
}

fn reverse_string(s: &mut String) {
    let rev = s.chars().rev().collect();
    *s = rev;
}

fn split_idents_on_dot(line: &str) -> Option<(usize, Vec<String>)> {
    let mut words = vec![String::new()];
    let mut startpos = 0;
    for (i, c) in line.chars().rev().enumerate() {
        match c {
            '.' => {
                if i != 0 && words.last().is_some_and(|s| s.is_empty()) {
                    return None;
                }
                reverse_string(words.last_mut().unwrap());
                if words.len() == 1 {
                    startpos = line.len() - i;
                }
                words.push(String::new());
            }
            c if c.is_alphanumeric() || c == '_' => words.last_mut().unwrap().push(c),
            _ => {
                if words.len() == 1 {
                    if words.last().unwrap().is_empty() {
                        return None;
                    }
                    startpos = line.len() - i;
                }
                break;
            }
        }
    }
    if words.len() == 1 && words[0].is_empty() {
        return None;
    }
    reverse_string(words.last_mut().unwrap());
    words.reverse();

    Some((startpos, words))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_vm(f: impl FnOnce(&VirtualMachine, &PyDictRef)) {
        let interp = crate::new_interpreter();
        interp.enter(|vm| {
            let scope = vm.new_scope_with_main().unwrap();
            f(vm, &scope.globals);
        });
    }

    #[test]
    fn complete_builtin_name() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "pri").unwrap();
            assert_eq!(start, 0);
            assert!(candidates.contains(&"print".to_owned()));
        });
    }

    #[test]
    fn complete_keyword() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "de").unwrap();
            assert_eq!(start, 0);
            assert!(candidates.contains(&"def".to_owned()));
        });
    }

    #[test]
    fn complete_keyword_for() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "fo").unwrap();
            assert_eq!(start, 0);
            assert!(candidates.contains(&"for".to_owned()));
        });
    }

    #[test]
    fn complete_builtin_attribute() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "str.up").unwrap();
            assert_eq!(start, 4);
            assert!(candidates.contains(&"upper".to_owned()));
        });
    }

    #[test]
    fn complete_empty_line_shows_globals_and_builtins() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "").unwrap();
            assert_eq!(start, 0);
            assert!(candidates.contains(&"print".to_owned()));
            assert!(candidates.contains(&"int".to_owned()));
        });
    }

    #[test]
    fn tab_action_empty_line_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "", 0);
            assert!(
                matches!(action, TabAction::Complete(0, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_multiline_under_indented_indents() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n", 9);
            assert!(matches!(action, TabAction::Indent(ref s) if s == "    "));
        });
    }

    #[test]
    fn tab_action_multiline_partially_indented() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n  ", 11);
            assert!(matches!(action, TabAction::Indent(ref s) if s == "  "));
        });
    }

    #[test]
    fn tab_action_multiline_correctly_indented_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    ", 13);
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_continuation_under_indented_indents() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    x = 1\n", 19);
            assert!(matches!(action, TabAction::Indent(ref s) if s == "    "));
        });
    }

    #[test]
    fn tab_action_continuation_correctly_indented_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    x = 1\n    ", 23);
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_single_line_whitespace_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "    ", 4);
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }
}
