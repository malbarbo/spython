//! The `NameError` and the `UnboundLocalError` of a name without a value,
//! with the suggestion of CPython.

use std::collections::BTreeMap;

use super::Lowering;
use crate::cpython::{names, suggest};
use crate::ir::{Binding, Group, Limit, NameError, Rep, VarId};

impl<'a> Lowering<'a, '_> {
    /// The `NameError` of `name` in the frame that the lowering is in, with
    /// the candidates of the suggestion of CPython. Each candidate that the
    /// program asks about gets a flag.
    pub(super) fn name_error(&self, name: &str) -> NameError {
        let mut scope = self.scope.borrow_mut();
        let mut module = self.vars.borrow_mut();
        let mut funcs = self.funcs.borrow_mut();
        let locals: Vec<(VarId, String)> = scope.as_ref().map_or_else(Vec::new, |scope| {
            scope
                .order
                .iter()
                .filter_map(|local| Some((scope.vars.find(local)?.0, local.clone())))
                .collect()
        });
        let globals: Vec<(Binding, String)> = module
            .names()
            .into_iter()
            .map(|(id, global)| (Binding::Global(id), global.to_owned()))
            .chain(
                funcs
                    .ids
                    .iter()
                    .map(|(func, id)| (Binding::Func(*id), func.clone())),
            )
            .collect();

        // CPython lists the locals, the globals and the builtins, and the
        // frame of the module lists its globals as its locals too.
        let (factor, params) = match scope.as_ref() {
            Some(scope) => {
                let id = funcs
                    .ids
                    .get(&scope.name)
                    .expect("the function of the scope");
                (1, funcs.get(*id).params.len())
            }
            None => (2, 0),
        };
        let most = locals.len()
            + factor * (names::MODULE_GLOBALS.len() + globals.len())
            + names::BUILTINS.len();
        let limit = (most >= suggest::MAX_CANDIDATE_ITEMS).then(|| Limit {
            max: count(suggest::MAX_CANDIDATE_ITEMS),
            always: count(params + factor * names::MODULE_GLOBALS.len() + names::BUILTINS.len()),
            globals: count(factor),
            locals: locals.iter().skip(params).map(|(id, _)| *id).collect(),
        });

        let mut by_distance: BTreeMap<usize, Group> = BTreeMap::new();
        for (id, local) in &locals {
            if let Some(group) = group_of(&mut by_distance, name, local) {
                group.locals.push((*id, local.clone()));
            }
        }
        // The globals of the module come before the globals of the program.
        for global in names::MODULE_GLOBALS {
            if let Some(group) = group_of(&mut by_distance, name, global) {
                group.always.get_or_insert_with(|| (*global).to_owned());
            }
        }
        for (binding, global) in &globals {
            if let Some(group) = group_of(&mut by_distance, name, global)
                && group.always.is_none()
            {
                group.globals.push((*binding, global.clone()));
            }
        }
        for builtin in names::BUILTINS {
            if let Some(group) = group_of(&mut by_distance, name, builtin) {
                group.always.get_or_insert_with(|| (*builtin).to_owned());
            }
        }
        let mut groups = Vec::new();
        for group in by_distance.into_values() {
            let last = group.always.is_some();
            groups.push(group);
            if last {
                break;
            }
        }

        let self_attr = match scope.as_ref() {
            Some(scope) => scope
                .vars
                .find("self")
                .filter(|_| scope.is_local("self"))
                .filter(|(_, rep)| has_attribute(Some(*rep), name))
                .map(|(id, _)| Binding::Local(id)),
            None => match (module.find("self"), funcs.ids.get("self")) {
                (Some((id, rep)), _) if has_attribute(Some(rep), name) => Some(Binding::Global(id)),
                (_, Some(id)) if has_attribute(None, name) => Some(Binding::Func(*id)),
                _ => None,
            },
        };

        let mut flag = |binding: Binding, ordered: bool| match binding {
            Binding::Local(id) => {
                if let Some(scope) = scope.as_mut() {
                    scope.vars.get_mut(id).flagged = true;
                }
            }
            Binding::Global(id) => {
                let var = module.get_mut(id);
                var.flagged = true;
                var.ordered |= ordered;
            }
            Binding::Func(id) => {
                *funcs
                    .ordered
                    .get_mut(id.index())
                    .expect("a function of the module") |= ordered;
            }
        };
        // The number of globals with a value is the number of ordered
        // globals with a value.
        let all_ordered = limit.is_some();
        if all_ordered {
            for (binding, _) in &globals {
                flag(*binding, true);
            }
        }
        for local in limit.iter().flat_map(|limit| &limit.locals) {
            flag(Binding::Local(*local), false);
        }
        for group in &groups {
            for (id, _) in &group.locals {
                flag(Binding::Local(*id), false);
            }
            for (binding, _) in &group.globals {
                flag(*binding, group.globals.len() > 1);
            }
        }
        if let Some(binding) = self_attr {
            flag(binding, false);
        }
        let base = format!("NameError: name '{name}' is not defined");
        let (import, or_import) = if names::STDLIB_MODULES.binary_search(&name).is_ok() {
            (
                format!(". Did you forget to import '{name}'?"),
                format!(" Or did you forget to import '{name}'?"),
            )
        } else {
            (String::new(), String::new())
        };
        NameError {
            message: format!("{base}{import}"),
            suggested: (format!("{base}. Did you mean: '"), format!("'?{or_import}")),
            self_attr: self_attr.map(|binding| (binding, format!("self.{name}"))),
            limit,
            groups,
        }
    }
}

/// Returns `true` if a value of `rep` has the attribute `name`, `false`
/// otherwise. A `rep` of `None` stands for a function.
fn has_attribute(rep: Option<Rep>, name: &str) -> bool {
    let attributes = match rep {
        Some(Rep::Int) => names::INT_ATTRIBUTES,
        Some(Rep::Float) => names::FLOAT_ATTRIBUTES,
        Some(Rep::Bool) => names::BOOL_ATTRIBUTES,
        Some(Rep::Str) => names::STR_ATTRIBUTES,
        Some(Rep::None) => unreachable!("a variable of rep None"),
        None => names::FUNCTION_ATTRIBUTES,
    };
    attributes.binary_search(&name).is_ok()
}

/// The group of the distance from `name` to `candidate`, and `None` when
/// CPython would not suggest `candidate` for `name`.
fn group_of<'m>(
    groups: &'m mut BTreeMap<usize, Group>,
    name: &str,
    candidate: &str,
) -> Option<&'m mut Group> {
    if candidate == name {
        return None;
    }
    let max = suggest::max_distance(name, candidate);
    let distance = suggest::distance(name, candidate, max);
    (distance <= max).then(|| {
        groups.entry(distance).or_insert_with(|| Group {
            locals: Vec::new(),
            globals: Vec::new(),
            always: None,
        })
    })
}

pub(super) fn unbound_local(name: &str) -> String {
    format!(
        "UnboundLocalError: cannot access local variable '{name}' where it is not \
         associated with a value"
    )
}

fn count(n: usize) -> u32 {
    u32::try_from(n).expect("fewer than 2^32 names")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::group_of;
    use crate::ir::VarId;

    /// The candidate that the `NameError` suggests for `name` when every
    /// candidate has a value, the first one of the nearest group.
    fn suggestion<'a>(candidates: &[&'a str], name: &str) -> Option<&'a str> {
        let mut groups = BTreeMap::new();
        for candidate in candidates {
            if let Some(group) = group_of(&mut groups, name, candidate) {
                group.locals.push((VarId::new(0), (*candidate).to_owned()));
            }
        }
        let (_, group) = groups.into_iter().next()?;
        let (_, first) = group.locals.into_iter().next()?;
        candidates
            .iter()
            .copied()
            .find(|candidate| *candidate == first)
    }

    /// Each name with its candidates, and what CPython 3.13 suggests.
    #[test]
    fn suggests_what_cpython_suggests() {
        assert_eq!(suggestion(&["abd", "abe"], "abc"), Some("abd"));
        assert_eq!(suggestion(&["xyz"], "abc"), None);
        assert_eq!(suggestion(&["total", "totals"], "Total"), Some("total"));
        assert_eq!(suggestion(&["Total", "tota"], "total"), Some("Total"));
        assert_eq!(suggestion(&["round", "counter"], "count"), Some("round"));
        assert_eq!(suggestion(&["name", "nomes", "none"], "nome"), Some("name"));
        assert_eq!(suggestion(&["y", "xx"], "x"), Some("xx"));
        assert_eq!(
            suggestion(
                &["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaax"],
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaay"
            ),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaax")
        );
        assert_eq!(
            suggestion(
                &[
                    "yayayayayayayayayayayayayayayayayayayayaya",
                    "xaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxab"
                ],
                "xaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxa"
            ),
            Some("xaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxaxab")
        );
        assert_eq!(suggestion(&["cafe", "cafè"], "café"), Some("cafè"));
        assert_eq!(suggestion(&["abc", "abd"], "abc"), Some("abd"));
    }
}
