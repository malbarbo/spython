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

/// The keywords that ask for a word after them, which the completion offers
/// with the space that opens it. The rest end the line, like `return`, or take
/// a colon next, like `else`.
pub const PYTHON_KEYWORDS_OPEN: &[&str] = &[
    "and", "as", "assert", "async", "await", "case", "class", "def", "del", "elif", "for", "from",
    "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or", "while",
    "with",
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
    commands: &[(&str, bool)],
) -> TabAction {
    let before = &text[..cursor_pos];
    let current_line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let current_line = &before[current_line_start..];

    if current_line.bytes().all(|b| b == b' ') {
        let current_indent = current_line.len();
        // The same question Enter asks, so the two keys put the line in the
        // same place: the whole text and where the caret is in it.
        let expected = crate::input::indent_at(text, cursor_pos);
        if current_indent < expected {
            return TabAction::Indent(" ".repeat(expected - current_indent));
        }
        // At or above expected indent → show all completions.
        return match complete(vm, globals, "") {
            Some((_, candidates)) => TabAction::Complete(cursor_pos, candidates),
            None => TabAction::Nothing,
        };
    }

    if let Some(candidates) = complete_command(current_line, commands) {
        let indent = current_line.len() - current_line.trim_start().len();
        return TabAction::Complete(current_line_start + indent, candidates);
    }

    match complete(vm, globals, before) {
        Some((startpos, candidates)) => TabAction::Complete(startpos, candidates),
        None => TabAction::Nothing,
    }
}

/// The commands a line starting with a colon can still become. `None` where a
/// command cannot go, and once one is typed and what it takes is being written.
fn complete_command(line: &str, commands: &[(&str, bool)]) -> Option<Vec<String>> {
    let prefix = line.trim_start();
    if !prefix.starts_with(':') || prefix.contains(char::is_whitespace) {
        return None;
    }
    Some(
        commands
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            // A command that takes something is offered with the space that
            // opens it, the way `import ` is.
            .map(|(name, takes_arg)| {
                if *takes_arg {
                    format!("{name} ")
                } else {
                    (*name).to_owned()
                }
            })
            .collect(),
    )
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
                result.push(if PYTHON_KEYWORDS_OPEN.contains(kw) {
                    format!("{kw} ")
                } else {
                    (*kw).to_owned()
                });
            }
        }
    }

    result.sort();

    Some((startpos, result))
}

fn get_available_completions<'a>(
    vm: &'a VirtualMachine,
    globals: &PyDictRef,
    words: &'a [String],
) -> Option<(&'a str, impl Iterator<Item = PyResult<PyStrRef>> + 'a)> {
    let (first, rest) = words.split_first()?;

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
    // `words` starts with one word and only ever grows, so it always has a last.
    const NON_EMPTY: &str = "words always has a last word";
    let mut words = vec![String::new()];
    let mut startpos = 0;
    for (char_count, (byte_offset, c)) in line.char_indices().rev().enumerate() {
        match c {
            '.' => {
                if char_count != 0 && words.last().is_some_and(|s| s.is_empty()) {
                    return None;
                }
                reverse_string(words.last_mut().expect(NON_EMPTY));
                if words.len() == 1 {
                    startpos = byte_offset + c.len_utf8();
                }
                words.push(String::new());
            }
            c if c.is_alphanumeric() || c == '_' => {
                words.last_mut().expect(NON_EMPTY).push(c);
            }
            _ => {
                if words.len() == 1 {
                    if words.last().expect(NON_EMPTY).is_empty() {
                        return None;
                    }
                    startpos = byte_offset + c.len_utf8();
                }
                break;
            }
        }
    }
    if words.len() == 1 && words.last().expect(NON_EMPTY).is_empty() {
        return None;
    }
    reverse_string(words.last_mut().expect(NON_EMPTY));
    words.reverse();

    Some((startpos, words))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The commands as a session hands them over, so the tests complete the
    /// same list the frontends do.
    fn commands() -> Vec<(&'static str, bool)> {
        crate::shell::Shell::new(crate::repl::Repl::new(crate::Level::Full, false)).command_names()
    }

    fn with_vm(f: impl FnOnce(&VirtualMachine, &PyDictRef)) {
        let interp = crate::run::new_interpreter();
        interp.enter(|vm| {
            let scope = vm.new_scope_with_main().expect("a fresh __main__ scope");
            f(vm, &scope.globals);
        });
    }

    #[test]
    fn complete_builtin_name() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "pri").expect("a completion for pri");
            assert_eq!(start, 0);
            assert!(candidates.contains(&"print".to_owned()));
        });
    }

    #[test]
    fn complete_keyword() {
        with_vm(|vm, globals| {
            // A name follows `def`, so the space that opens it comes along.
            let (start, candidates) = complete(vm, globals, "de").expect("a completion for de");
            assert_eq!(start, 0);
            assert!(candidates.contains(&"def ".to_owned()));
        });
    }

    #[test]
    fn complete_keyword_for() {
        with_vm(|vm, globals| {
            let (start, candidates) = complete(vm, globals, "fo").expect("a completion for fo");
            assert_eq!(start, 0);
            assert!(candidates.contains(&"for ".to_owned()));
        });
    }

    #[test]
    fn complete_keyword_that_ends_the_line() {
        with_vm(|vm, globals| {
            // `return` and `pass` can be the whole line, and `else` takes a
            // colon, which no space goes in front of.
            let (_, candidates) = complete(vm, globals, "ret").expect("a completion for ret");
            assert!(candidates.contains(&"return".to_owned()));
            let (_, candidates) = complete(vm, globals, "pas").expect("a completion for pas");
            assert!(candidates.contains(&"pass".to_owned()));
            let (_, candidates) = complete(vm, globals, "els").expect("a completion for els");
            assert!(candidates.contains(&"else".to_owned()));
        });
    }

    #[test]
    fn every_open_keyword_is_a_keyword() {
        for kw in PYTHON_KEYWORDS_OPEN {
            assert!(PYTHON_KEYWORDS.contains(kw), "{kw} is not a keyword");
        }
    }

    #[test]
    fn complete_builtin_attribute() {
        with_vm(|vm, globals| {
            let (start, candidates) =
                complete(vm, globals, "str.up").expect("a completion for str.up");
            assert_eq!(start, 4);
            assert!(candidates.contains(&"upper".to_owned()));
        });
    }

    #[test]
    fn complete_empty_line_shows_globals_and_builtins() {
        with_vm(|vm, globals| {
            let (start, candidates) =
                complete(vm, globals, "").expect("completions for an empty line");
            assert_eq!(start, 0);
            assert!(candidates.contains(&"print".to_owned()));
            assert!(candidates.contains(&"int".to_owned()));
        });
    }

    #[test]
    fn tab_action_empty_line_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "", 0, &commands());
            assert!(
                matches!(action, TabAction::Complete(0, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_multiline_under_indented_indents() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n", 9, &commands());
            assert!(matches!(action, TabAction::Indent(ref s) if s == "    "));
        });
    }

    #[test]
    fn tab_action_multiline_partially_indented() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n  ", 11, &commands());
            assert!(matches!(action, TabAction::Indent(ref s) if s == "  "));
        });
    }

    #[test]
    fn tab_action_multiline_correctly_indented_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    ", 13, &commands());
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_continuation_under_indented_indents() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    x = 1\n", 19, &commands());
            assert!(matches!(action, TabAction::Indent(ref s) if s == "    "));
        });
    }

    #[test]
    fn tab_action_continuation_correctly_indented_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "def f():\n    x = 1\n    ", 23, &commands());
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_completes_a_command() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, ":ti", 3, &commands());
            assert!(matches!(action, TabAction::Complete(0, ref c) if c == &[":time "]));
        });
    }

    #[test]
    fn tab_action_offers_no_name_where_a_command_goes() {
        with_vm(|vm, globals| {
            // `type` is a builtin, and after a colon it is not what is being
            // asked for.
            let action = tab_action(vm, globals, ":ty", 3, &commands());
            assert!(matches!(action, TabAction::Complete(0, ref c) if c == &[":type "]));
        });
    }

    #[test]
    fn tab_action_lists_every_command_after_a_bare_colon() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, ":", 1, &commands());
            let TabAction::Complete(0, candidates) = action else {
                panic!("a bare colon should offer the commands");
            };
            assert_eq!(candidates.len(), commands().len());
            // One that takes something is offered with the space that opens it.
            assert!(candidates.contains(&":help".to_owned()));
            assert!(candidates.contains(&":type ".to_owned()));
        });
    }

    #[test]
    fn tab_action_completes_a_command_that_is_indented() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "  :h", 4, &commands());
            assert!(matches!(action, TabAction::Complete(2, ref c) if c == &[":help"]));
        });
    }

    #[test]
    fn tab_action_completes_what_a_command_takes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, ":type pri", 9, &commands());
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }

    #[test]
    fn tab_action_single_line_whitespace_completes() {
        with_vm(|vm, globals| {
            let action = tab_action(vm, globals, "    ", 4, &commands());
            assert!(
                matches!(action, TabAction::Complete(_, ref c) if c.contains(&"print".to_owned()))
            );
        });
    }
}
