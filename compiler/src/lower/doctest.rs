//! The doctests of a file. The checker gives the examples of each docstring
//! as the body of a function, and the lowering runs each example on its
//! own, as the runner of spython does.

use ruff_python_ast as ast;
use ruff_text_size::Ranged;

use super::Lowering;
use crate::cpython::traceback;
use crate::ir::{ArithOp, CmpOp, Expr, Place, Rep, Site, Stmt, VarId};
use crate::{Docstring, Error, Example};

/// What the lowering keeps for a program that tests its file.
pub(super) struct Tests<'a> {
    pub docstrings: &'a [Docstring],
    /// The variables of the module that hold a copy of a name of the
    /// module for the examples.
    pub copies: &'a [String],
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
        let shown = |text: &str| Expr::Str(text.replace('\n', "\n      "));
        let args = vec![
            shown(example.source.trim_end_matches('\n')),
            shown(example.want.strip_suffix('\n').unwrap_or(&example.want)),
            Expr::Str(example.want.clone()),
            Expr::Str(example.exc_msg.clone().unwrap_or_default()),
            Expr::Bool(example.exc_msg.is_some()),
            Expr::Bool(tests.verbose),
        ];
        let site = self.hidden_site();
        let report = self.prelude_call_at("doctest", "example", args, site)?;
        // 1 is the outcome of an example that fails, and 2 of one that
        // raises.
        let count = |counter, counted| Stmt::Assign {
            targets: vec![Place::Global(counter)],
            value: Expr::Arith {
                op: ArithOp::Add,
                left: Box::new(load_int(counter)),
                right: Box::new(Expr::BoolAsInt(Box::new(Expr::Compare(
                    Box::new(load_int(outcome)),
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
                order: None,
                rep: Rep::None,
                site: Some(site),
                check: None,
            }));
        }
        let total = Expr::Int(i32::try_from(total).expect("fewer than 2^31 examples"));
        let args = vec![total, load_int(tests.failed), load_int(tests.errors)];
        let failures = self.prelude_call_at("doctest", "summary", args, site)?;
        stmts.push(Stmt::Exit(failures));
        Ok(stmts)
    }

    /// A new site whose frame a traceback does not show, for the code that
    /// the lowering adds to run the doctests.
    fn hidden_site(&self) -> Site {
        self.add_site(String::new(), 0, String::new())
    }
}

/// The value of `var`, a global `int` of the doctests that always has one.
fn load_int(var: VarId) -> Expr {
    Expr::Load {
        place: Place::Global(var),
        rep: Rep::Int,
        check: None,
    }
}
