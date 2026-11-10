//! The doctests of a file. The checker gives the examples of each docstring
//! as the body of a function, and the lowering runs each example on its
//! own, as the runner of spython does.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::Lowering;
use crate::cpython::traceback;
use crate::ir::{ArithOp, CmpOp, Expr, Place, Rep, Site, SiteFrame, Stmt, VarId};
use crate::{Docstring, Error, Example};

/// What the lowering keeps for a program that tests its file.
pub(super) struct Tests<'a> {
    pub docstrings: &'a [Docstring],
    pub verbose: bool,
    /// The variable of the module that counts the examples that fail.
    pub failed: VarId,
    /// The variable of the module that counts the examples that raise an
    /// exception that they do not expect.
    pub errors: VarId,
    /// The variable of the module that holds the outcome of the last
    /// example, as `example` of the prelude gives it.
    pub outcome: VarId,
}

/// The docstring of the function that the lowering is in.
pub(super) struct DoctestScope {
    /// The name of what the docstring belongs to.
    pub owner: String,
    /// The index of the example that the lowering is in and the line where
    /// its code starts, and `None` outside every example.
    pub example: Option<(usize, usize)>,
}

impl<'a> Lowering<'a, '_> {
    /// Returns `true` if the lowering is in the code of the examples of a
    /// docstring, `false` otherwise.
    pub(super) fn in_doctest(&self) -> bool {
        self.scope
            .borrow()
            .as_ref()
            .is_some_and(|scope| scope.doctest.is_some())
    }

    /// The body of the function of `docstring`, where each example runs on
    /// its own and reports its outcome.
    pub(super) fn doctest_body(
        &self,
        def: &ast::StmtFunctionDef,
        docstring: &Docstring,
    ) -> Result<Vec<Stmt>, Error> {
        let lines = &self.unit.get().lines;
        let line_of = |stmt: &ast::Stmt| traceback::line(lines, stmt.range());
        let mut stmts = def.body.iter().peekable();
        let mut lowered = Vec::new();
        for (index, example) in docstring.examples.iter().enumerate() {
            // The copies of the names of the module come before the first
            // example.
            while let Some(stmt) = stmts.next_if(|stmt| line_of(stmt) < example.line) {
                lowered.extend(self.body(std::slice::from_ref(stmt))?);
            }
            let end = docstring
                .examples
                .get(index + 1)
                .map_or(usize::MAX, |next| next.line);
            self.set_example(Some((index, example.line)));
            let mut body = Vec::new();
            while let Some(stmt) = stmts.next_if(|stmt| line_of(stmt) < end) {
                body.extend(self.body(std::slice::from_ref(stmt))?);
            }
            self.set_example(None);
            lowered.push(Stmt::Example(body));
            lowered.extend(self.report(example)?);
        }
        Ok(lowered)
    }

    fn set_example(&self, example: Option<(usize, usize)>) {
        if let Some(doctest) = self
            .scope
            .borrow_mut()
            .as_mut()
            .and_then(|scope| scope.doctest.as_mut())
        {
            doctest.example = example;
        }
    }

    /// The report of `example`, which has just run, and the count of its
    /// outcome.
    fn report(&self, example: &Example) -> Result<Vec<Stmt>, Error> {
        let tests = self.tests.as_ref().expect("a program that tests its file");
        let (failed, errors, outcome) = (tests.failed, tests.errors, tests.outcome);
        let want = to_ascii(&example.want);
        let exc = example.exc_msg.as_deref().map(to_ascii).unwrap_or_default();
        let shown = |text: &str| Expr::Str(text.replace('\n', "\n      "));
        let args = vec![
            shown(example.source.trim_end_matches('\n')),
            shown(example.want.strip_suffix('\n').unwrap_or(&example.want)),
            Expr::Str(want.clone()),
            Expr::Str(blank_lines(&want)),
            Expr::Str(exc.clone()),
            Expr::Str(blank_lines(&exc)),
            Expr::Bool(example.exc_msg.is_some()),
            Expr::Bool(tests.verbose),
        ];
        let site = self.hidden_site();
        let report = self.prelude_call_at("doctest", "example", args, site)?;
        let load = |var| Expr::Load {
            place: Place::Global(var),
            rep: Rep::Int,
            check: None,
        };
        // 1 is the outcome of an example that fails, and 2 of one that
        // raises.
        let count = |counter, counted| Stmt::Assign {
            targets: vec![Place::Global(counter)],
            value: Expr::Arith {
                op: ArithOp::Add,
                left: Box::new(load(counter)),
                right: Box::new(Expr::BoolAsInt(Box::new(Expr::Compare(
                    Box::new(load(outcome)),
                    vec![(CmpOp::Eq, Expr::Int(counted))],
                )))),
                site,
            },
        };
        Ok(vec![
            Stmt::Assign {
                targets: vec![Place::Global(outcome)],
                value: report,
            },
            count(failed, 1),
            count(errors, 2),
        ])
    }

    /// The code that runs the doctests after the body of the file. The
    /// docstring of the module comes first, and the others in the order of
    /// their names, as in the runner.
    pub(super) fn tests_driver(&self) -> Result<Vec<Stmt>, Error> {
        let tests = self.tests.as_ref().expect("a program that tests its file");
        if tests.docstrings.is_empty() {
            return Ok(Vec::new());
        }
        let site = self.hidden_site();
        let call = |name: &str, args| {
            self.prelude_call_at("doctest", name, args, site)
                .map(Stmt::Expr)
        };
        let zero = |var| Stmt::Assign {
            targets: vec![Place::Global(var)],
            value: Expr::Int(0),
        };
        let load = |var| Expr::Load {
            place: Place::Global(var),
            rep: Rep::Int,
            check: None,
        };
        let mut stmts = vec![
            zero(tests.failed),
            zero(tests.errors),
            call("start", Vec::new())?,
        ];
        let mut docstrings: Vec<&Docstring> = tests.docstrings.iter().collect();
        docstrings.sort_by_key(|docstring| (docstring.owner != "<module>", &docstring.owner));
        let mut total = 0;
        for docstring in docstrings {
            let func = *self
                .funcs
                .borrow()
                .names
                .get(&docstring.function)
                .expect("the function of a docstring");
            total += docstring.examples.len();
            stmts.push(call("group", vec![Expr::Str(docstring.owner.clone())])?);
            stmts.push(Stmt::Expr(Expr::Call {
                func,
                args: Vec::new(),
                order: Vec::new(),
                rep: Rep::None,
                site: Some(site),
                check: None,
            }));
        }
        let total = Expr::Int(i32::try_from(total).expect("fewer than 2^31 examples"));
        let args = vec![total, load(tests.failed), load(tests.errors)];
        let failures = self.prelude_call_at("doctest", "summary", args, site)?;
        stmts.push(Stmt::Exit(failures));
        Ok(stmts)
    }

    /// A new site whose frame a traceback does not show, for the code that
    /// the lowering adds to run the doctests.
    fn hidden_site(&self) -> Site {
        let mut sites = self.sites.borrow_mut();
        let mut lines = self.lines.borrow_mut();
        let count = u32::try_from(lines.len()).expect("fewer than 2^32 lines");
        let line = *lines.entry((String::new(), 0)).or_insert(count);
        sites.push(SiteFrame {
            text: String::new(),
            line,
        });
        Site::new(sites.len() - 1)
    }
}

/// `text` with an escape in the place of each character that is not ASCII,
/// as `str.encode('ASCII', 'backslashreplace')` writes it.
fn to_ascii(text: &str) -> String {
    let mut ascii = String::new();
    for c in text.chars() {
        match u32::from(c) {
            _ if c.is_ascii() => ascii.push(c),
            code @ ..0x100 => ascii.push_str(&format!("\\x{code:02x}")),
            code @ ..0x1_0000 => ascii.push_str(&format!("\\u{code:04x}")),
            code => ascii.push_str(&format!("\\U{code:08x}")),
        }
    }
    ascii
}

/// The ASCII text `want` with an empty line in the place of each line that
/// is `<BLANKLINE>` and spaces, which is how the output asks for a blank
/// line.
fn blank_lines(want: &str) -> String {
    // `str.isspace` of an ASCII character.
    let is_space = |c: char| matches!(c, ' ' | '\t'..='\r' | '\x1c'..='\x1f');
    want.split('\n')
        .map(|line| match line.strip_prefix("<BLANKLINE>") {
            Some(rest) if rest.chars().all(is_space) => "",
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{blank_lines, to_ascii};

    #[test]
    fn the_output_of_an_example_is_compared_in_ascii() {
        assert_eq!(
            to_ascii("a\\é\u{100}\u{1f600}\n"),
            "a\\\\xe9\\u0100\\U0001f600\n"
        );
        assert_eq!(
            blank_lines("a\n<BLANKLINE>\n<BLANKLINE> \t\n<BLANKLINE>x\n <BLANKLINE>\n"),
            "a\n\n\n<BLANKLINE>x\n <BLANKLINE>\n"
        );
    }
}
