//! The exceptions, and the traceback of an exception that nothing catches.

use std::cell::RefCell;
use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, StorageType, ValType};

use crate::cpython::{
    Class, FLUSH_FAILED, FLUSH_FAILED_EXIT, MORE_TIME, MORE_TIMES, Message, OS_ERRORS,
    RAISED_BEFORE_TESTS, RECURSION_LIMIT, RECURSIVE_CUTOFF, REPEATED, TRACEBACK,
    exception::last_line,
};
use crate::emit::module::{
    FuncIdx, FuncState, GlobalIdx, Ins, Label, LocalIdx, ModuleState, TagIdx, TypeIdx,
};
use crate::emit::repr::str::StrRepr;
use crate::emit::runtime::host::{Capture, Host};
use crate::ir::SiteFrame;

pub(in crate::emit) struct Exceptions {
    /// `(struct (field $class (ref $Class)) (field $arg (ref null $Str))
    /// (field $tb (mut (ref null $Frame))))`. The argument is null for an
    /// exception without one.
    exc: TypeIdx,
    /// `(struct (field $name (ref $Str)) (field $base (ref null $Class)))`,
    /// a class of exception.
    class: TypeIdx,
    /// The global of each class that the module raises, and of its bases.
    classes: RefCell<HashMap<Class, GlobalIdx>>,
    /// The tag of every exception, whose payload is a `(ref $Exc)`.
    tag: TagIdx,
    /// `(func (param (ref null $Str) (ref $Class)))`, which raises an
    /// exception of the class with the argument.
    raise: FuncIdx,
    /// `(func (param i32))`, which raises the `OSError` of the error of
    /// WASI.
    os_error: FuncIdx,
    /// `(func (param (ref $Exc) i32))`, which adds the frame of the site to
    /// the traceback, writes the traceback to the standard error, and ends
    /// the program with 1.
    uncaught: FuncIdx,
    /// `(func (result i32))`, which flushes the standard output and returns
    /// the error of WASI, or 0.
    flush: FuncIdx,
    /// `(func (param i32))`, which ends the program with the exit code, as
    /// `Py_Finalize` does.
    exit: FuncIdx,
    /// `(func (param (ref $Exc) i32))`, which adds the frame of the site to
    /// the traceback and raises the exception again.
    reraise: FuncIdx,
    /// `(array (mut i32))`, with three numbers for each site. They are the
    /// id of the line of the frame of the site, and the start and the
    /// length of the text of the frame in the data that [`Self::finish`]
    /// writes.
    sites: TypeIdx,
    /// The global that holds the array of the sites.
    sites_global: GlobalIdx,
    /// `(func (param i32) (result (ref $Str)))`, which builds the text of
    /// the frame of the site. [`Self::finish`] defines it, once it knows
    /// the texts.
    text: FuncIdx,
    /// The global with the number of frames, which starts at 1 for the
    /// frame of the module.
    depth: GlobalIdx,
    /// What a program that runs doctests has for them.
    tests: Option<Tests>,
}

#[derive(Clone, Copy)]
struct Tests {
    /// The global that is 1 once the tests run.
    running: GlobalIdx,
    /// The global with what the last example wrote, a `(ref null $Str)`.
    got: GlobalIdx,
    /// The global with the last line of the traceback of the exception of
    /// the last example, or with an empty `str`.
    raised: GlobalIdx,
    /// `(func (param (ref $Exc)))`, which takes the exception that ended an
    /// example. It sets `raised`, or raises the exception again when the
    /// exception stops the program.
    caught: FuncIdx,
}

impl Exceptions {
    /// The exceptions of a module, which runs doctests when `tests` is
    /// `true`.
    pub fn new(m: &mut ModuleState, host: &Host, str: &mut StrRepr, tests: bool) -> Self {
        let str_type = str.type_index();
        // `(struct (field $site i32) (field $next (ref null $Frame)))`, a
        // frame of the traceback, from the outermost one.
        let frame = m.next_type_index();
        m.struct_type(&[
            (StorageType::Val(ValType::I32), false),
            (StorageType::Val(frame.ref_to(true)), false),
        ]);
        let class = m.next_type_index();
        m.struct_type(&[
            (StorageType::Val(str_type.ref_to(false)), false),
            (StorageType::Val(class.ref_to(true)), false),
        ]);
        let mut classes = HashMap::new();
        let exc = m.struct_type(&[
            (StorageType::Val(class.ref_to(false)), false),
            (StorageType::Val(str_type.ref_to(true)), false),
            (StorageType::Val(frame.ref_to(true)), true),
        ]);
        let tag_type = m.func_type(&[exc.ref_to(false)], &[]);
        let tag = m.tag(tag_type);
        let sites = m.array_type(StorageType::Val(ValType::I32), true);
        let sites_global = m.global(sites.ref_to(true), &ConstExpr::ref_null(sites.heap_type()));
        let text_type = m.func_type(&[ValType::I32], &[str_type.ref_to(false)]);
        let text = m.reserve(text_type);
        let depth = m.global(ValType::I32, &ConstExpr::i32_const(1));
        let repeated = add_repeated(m, str);

        let raise_type = m.func_type(&[str_type.ref_to(true), class.ref_to(false)], &[]);
        let mut f = FuncState::new(2);
        let (arg, of) = (f.param(0), f.param(1));
        f.ins()
            .local_get(of)
            .local_get(arg)
            .ref_null(frame.heap_type())
            .struct_new(exc)
            .throw(tag);
        let raise = m.add_func(raise_type, f.finish());
        let os_error_line = add_os_error_line(m, str);
        let os_error_class = add_os_error_class(m, str, class, &mut classes);
        let os_error_type = m.func_type(&[ValType::I32], &[]);
        let mut f = FuncState::new(1);
        let errno = f.param(0);
        f.ins()
            .local_get(errno)
            .i32_const(0)
            .call(os_error_line)
            .local_get(errno)
            .call(os_error_class)
            .call(raise)
            .unreachable();
        let os_error = m.add_func(os_error_type, f.finish());
        let exit = add_exit(m, host, str, os_error_line);
        let tests = tests.then(|| {
            let null = ConstExpr::ref_null(str_type.heap_type());
            let raised = m.global(str_type.ref_to(true), &null);
            Tests {
                running: m.global(ValType::I32, &ConstExpr::i32_const(0)),
                got: m.global(str_type.ref_to(true), &null),
                raised,
                caught: add_caught(m, str, (exc, class, tag), &mut classes, raised),
            }
        });

        let frame_type = m.func_type(&[exc.ref_to(false), ValType::I32], &[]);
        // The frame of the site goes before the frames of the functions
        // that the code at the site called.
        let mut f = FuncState::new(2);
        let (e, site) = (f.param(0), f.param(1));
        f.ins()
            .local_get(e)
            .local_get(site)
            .local_get(e)
            .struct_get(exc, 2)
            .struct_new(frame)
            .struct_set(exc, 2);
        let add_frame = m.add_func(frame_type, f.finish());

        let mut f = FuncState::new(2);
        let (e, site) = (f.param(0), f.param(1));
        f.ins()
            .global_get(depth)
            .i32_const(1)
            .i32_sub()
            .global_set(depth)
            .local_get(e)
            .local_get(site)
            .call(add_frame)
            .local_get(e)
            .throw(tag);
        let reraise = m.add_func(frame_type, f.finish());

        // As `StackSummary.format` of CPython, a frame on the line of the
        // frame before it adds to the count of the run, and the traceback
        // writes the count at the end of the run.
        let mut f = FuncState::new(2);
        let (e, site) = (f.param(0), f.param(1));
        let next = f.local(frame.ref_to(true));
        let last = f.local(ValType::I32);
        let count = f.local(ValType::I32);
        let line = f.local(ValType::I32);
        f.ins()
            .local_get(e)
            .local_get(site)
            .call(add_frame)
            .call(host.flush)
            .drop()
            .call(host.to_stderr);
        str.literal(m, &mut f, TRACEBACK);
        str.write(&mut f);
        f.ins()
            .local_get(e)
            .struct_get(exc, 2)
            .local_set(next)
            .i32_const(-1)
            .local_set(last);
        let done = f.block(BlockType::Empty);
        let frames = f.loop_(BlockType::Empty);
        f.ins().local_get(next).ref_is_null();
        f.br_if(done);
        f.ins()
            .global_get(sites_global)
            .local_get(next)
            .struct_get(frame, 0)
            .i32_const(SITE_FIELDS)
            .i32_mul()
            .array_get(sites)
            .local_tee(line)
            .local_get(last)
            .i32_ne();
        f.if_(BlockType::Empty);
        f.ins()
            .local_get(count)
            .call(repeated)
            .local_get(line)
            .local_set(last)
            .i32_const(0)
            .local_set(count);
        f.end();
        f.ins()
            .local_get(count)
            .i32_const(1)
            .i32_add()
            .local_tee(count)
            .i32_const(RECURSIVE_CUTOFF)
            .i32_le_s();
        f.if_(BlockType::Empty);
        f.ins().local_get(next).struct_get(frame, 0).call(text);
        str.write(&mut f);
        f.end();
        f.ins().local_get(next).struct_get(frame, 1).local_set(next);
        f.br(frames);
        f.end();
        f.end();
        f.ins()
            .local_get(count)
            .call(repeated)
            .local_get(e)
            .struct_get(exc, 0)
            .struct_get(class, 0);
        str.write(&mut f);
        // CPython writes the class alone when the text of the argument is
        // empty.
        let arg = f.local(str_type.ref_to(true));
        let no_text = f.block(BlockType::Empty);
        f.ins()
            .local_get(e)
            .struct_get(exc, 1)
            .local_tee(arg)
            .ref_is_null();
        f.br_if(no_text);
        f.ins().local_get(arg).ref_as_non_null();
        str.truth(&mut f);
        f.ins().i32_eqz();
        f.br_if(no_text);
        str.literal(m, &mut f, ": ");
        str.write(&mut f);
        f.ins().local_get(arg).ref_as_non_null();
        str.write(&mut f);
        f.end();
        f.ins().i32_const('\n' as i32).call(host.write_char);
        if let Some(tests) = &tests {
            f.ins().global_get(tests.running).i32_eqz();
            f.if_(BlockType::Empty);
            str.literal(m, &mut f, RAISED_BEFORE_TESTS);
            str.write(&mut f);
            f.end();
        }
        f.ins().i32_const(1).call(exit);
        let uncaught = m.add_func(frame_type, f.finish());

        Exceptions {
            exc,
            class,
            classes: RefCell::new(classes),
            tag,
            raise,
            os_error,
            uncaught,
            flush: host.flush,
            exit,
            reraise,
            sites,
            sites_global,
            text,
            depth,
            tests,
        }
    }

    fn tests(&self) -> &Tests {
        self.tests.as_ref().expect("a program that runs doctests")
    }

    /// Marks that the tests run, so an exception from here on is not one of
    /// the body of the file.
    pub fn tests_run(&self, f: &mut FuncState) {
        f.ins().i32_const(1).global_set(self.tests().running);
    }

    /// Opens the body of an example, whose output goes to the buffer of
    /// `capture`. [`Self::end_example`] closes it with the label that this
    /// returns.
    pub fn begin_example(&self, f: &mut FuncState, capture: &Capture) -> Label {
        f.ins().i32_const(1).global_set(capture.on);
        let done = f.block(BlockType::Empty);
        let caught = f.block(BlockType::Result(self.exc.ref_to(false)));
        f.try_table(BlockType::Empty, self.tag, caught);
        done
    }

    /// Closes the body of an example, and keeps what it wrote and the
    /// exception that ended it.
    pub fn end_example(
        &self,
        m: &mut ModuleState,
        str: &mut StrRepr,
        f: &mut FuncState,
        capture: &Capture,
        done: Label,
    ) {
        let tests = *self.tests();
        f.end();
        str.literal(m, f, "");
        f.ins().global_set(tests.raised);
        f.br(done);
        f.end();
        f.ins().call(tests.caught);
        f.end();
        f.ins().i32_const(0).global_set(capture.on);
        str.captured(f);
        f.ins().global_set(tests.got);
    }

    /// Pushes what the last example wrote.
    pub fn captured(&self, f: &mut FuncState) {
        f.ins().global_get(self.tests().got).ref_as_non_null();
    }

    /// Pushes the last line of the traceback of the exception of the last
    /// example, with its `\n`, or an empty `str`.
    pub fn captured_exception(&self, f: &mut FuncState) {
        f.ins().global_get(self.tests().raised).ref_as_non_null();
    }

    /// Pops an `i32`, flushes the standard output as the end of a module
    /// does, and ends the program with the exit code 1 when the `i32` is not
    /// 0, and with 0 otherwise.
    pub fn exit(&self, f: &mut FuncState) {
        let code = f.temp(ValType::I32);
        f.ins()
            .i32_const(0)
            .i32_ne()
            .local_set(code)
            .call(self.flush)
            .drop()
            .local_get(code)
            .call(self.exit)
            .unreachable();
        f.free(code);
    }

    /// Pushes 1 when a call has room for one more frame, and 0 when the call
    /// raises `RecursionError`.
    pub fn call_fits(&self, f: &mut FuncState) {
        f.ins()
            .global_get(self.depth)
            .i32_const(RECURSION_LIMIT)
            .i32_lt_s();
    }

    /// Counts the frame of the function that starts.
    pub fn enter(&self, f: &mut FuncState) {
        self.add_depth(f, 1);
    }

    /// Drops the frame of the function that returns.
    pub fn leave(&self, f: &mut FuncState) {
        self.add_depth(f, -1);
    }

    fn add_depth(&self, f: &mut FuncState, delta: i32) {
        f.ins()
            .global_get(self.depth)
            .i32_const(delta)
            .i32_add()
            .global_set(self.depth);
    }

    /// Raises the exception of `message`.
    pub fn raise(
        &self,
        m: &mut ModuleState,
        str: &mut StrRepr,
        f: &mut FuncState,
        message: &Message,
    ) {
        if message.text.is_empty() {
            f.ins().ref_null(str.type_index().heap_type());
        } else {
            str.literal(m, f, message.text);
        }
        self.raise_text(m, str, f, message.class);
    }

    /// Pops a `str`, which may be null, and raises an exception of `class`
    /// with it as the text of the argument.
    pub fn raise_text(
        &self,
        m: &mut ModuleState,
        str: &mut StrRepr,
        f: &mut FuncState,
        class: Class,
    ) {
        let global = class_global(m, str, self.class, &mut self.classes.borrow_mut(), class);
        f.ins()
            .global_get(global)
            .ref_as_non_null()
            .call(self.raise)
            .unreachable();
    }

    /// Pops an error of WASI and raises its `OSError`.
    pub fn raise_os_error(&self, f: &mut FuncState) {
        f.ins().call(self.os_error).unreachable();
    }

    /// Opens the code of the module or of a function, where the handler
    /// that [`Self::end_module`] or [`Self::end_function`] adds catches an
    /// exception.
    pub fn begin(&self, f: &mut FuncState) {
        let caught = f.block(BlockType::Result(self.exc.ref_to(false)));
        f.try_table(BlockType::Empty, self.tag, caught);
    }

    /// Closes what [`Self::begin`] opened in the module, where an exception
    /// that nothing catches ends the program. `site` is the local that holds
    /// the site of the instruction that may raise. As in CPython, the
    /// standard output gets a flush that ignores an error at the end of the
    /// module, and the exit flushes it again.
    pub fn end_module(&self, f: &mut FuncState, site: LocalIdx) {
        f.end();
        f.ins()
            .call(self.flush)
            .drop()
            .i32_const(0)
            .call(self.exit)
            .unreachable();
        f.end();
        f.ins().local_get(site).call(self.uncaught);
    }

    /// Closes what [`Self::begin`] opened in a function, where an exception
    /// goes on to the caller with the frame of the function. Every path of
    /// the code of a function ends in a `return`.
    pub fn end_function(&self, f: &mut FuncState, site: LocalIdx) {
        f.end();
        f.ins().unreachable();
        f.end();
        f.ins().local_get(site).call(self.reraise).unreachable();
    }

    /// Fills the array of the sites, and defines [`Self::text`]. The
    /// texts stay as bytes until a traceback writes them, and two sites
    /// with the same text share its bytes.
    pub fn finish(&self, m: &mut ModuleState, str: &StrRepr, sites: &[SiteFrame]) {
        let mut texts = Vec::new();
        let mut places: HashMap<&str, [i32; 2]> = HashMap::new();
        let mut numbers = Vec::new();
        for site in sites {
            let [start, len] = *places.entry(&site.text).or_insert_with(|| {
                let start = i32::try_from(texts.len()).expect("texts shorter than 2^31 bytes");
                texts.extend(str.encode(&site.text));
                let len =
                    i32::try_from(site.text.chars().count()).expect("a text shorter than 2^31");
                [start, len]
            });
            let line = i32::try_from(site.line).expect("fewer than 2^31 lines");
            for number in [line, start, len] {
                numbers.extend(number.to_le_bytes());
            }
        }
        let texts = m.passive_data(texts);
        let numbers_len = i32::try_from(numbers.len() / 4).expect("fewer than 2^31 sites");
        let numbers = m.passive_data(numbers);
        Ins::new(&mut m.init)
            .i32_const(0)
            .i32_const(numbers_len)
            .array_new_data(self.sites, numbers)
            .global_set(self.sites_global)
            .data_drop(numbers);
        let mut f = FuncState::new(1);

        let site = f.param(0);
        for field in [1, 2] {
            f.ins()
                .global_get(self.sites_global)
                .local_get(site)
                .i32_const(SITE_FIELDS)
                .i32_mul()
                .i32_const(field)
                .i32_add()
                .array_get(self.sites);
        }
        str.new_data(&mut f, texts);
        m.define(self.text, f.finish());
    }
}

/// How many numbers [`Exceptions::finish`] writes for each site.
const SITE_FIELDS: i32 = 3;

/// `(func (param i32))`, which writes how many frames of a run the
/// traceback leaves out, when the run has more than the cutoff.
fn add_repeated(m: &mut ModuleState, str: &mut StrRepr) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let mut f = FuncState::new(1);
    let count = f.param(0);
    f.ins()
        .local_get(count)
        .i32_const(RECURSIVE_CUTOFF)
        .i32_gt_s();
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(count)
        .i32_const(RECURSIVE_CUTOFF)
        .i32_sub()
        .local_set(count);
    str.literal(m, &mut f, REPEATED);
    str.write(&mut f);
    f.ins().local_get(count).i64_extend_i32_s();
    str.decimal(&mut f);
    str.write(&mut f);
    f.ins().local_get(count).i32_const(1).i32_gt_s();
    f.if_(BlockType::Empty);
    str.literal(m, &mut f, MORE_TIMES);
    str.write(&mut f);
    f.else_();
    str.literal(m, &mut f, MORE_TIME);
    str.write(&mut f);
    f.end();
    f.end();
    m.add_func(ty, f.finish())
}

/// `(func (param (ref $Exc)))`, which takes the exception that ended an
/// example of a docstring. An exception of [`Class::Limit`] stops the
/// program, so the function raises it again. For any other, the function
/// sets `raised` to the last line of its traceback, with the `\n`.
fn add_caught(
    m: &mut ModuleState,
    str: &mut StrRepr,
    (exc, class, tag): (TypeIdx, TypeIdx, TagIdx),
    classes: &mut HashMap<Class, GlobalIdx>,
    raised: GlobalIdx,
) -> FuncIdx {
    let limit = class_global(m, str, class, classes, Class::Limit);
    let ty = m.func_type(&[exc.ref_to(false)], &[]);
    let mut f = FuncState::new(1);
    let e = f.param(0);
    let arg = f.local(str.val_type(true));
    let line = f.local(str.val_type(true));
    f.ins()
        .local_get(e)
        .struct_get(exc, 0)
        .global_get(limit)
        .ref_eq();
    f.if_(BlockType::Empty);
    f.ins().local_get(e).throw(tag);
    f.end();
    f.ins()
        .local_get(e)
        .struct_get(exc, 0)
        .struct_get(class, 0)
        .local_set(line);
    // The class alone is the line of an exception without a text.
    let no_text = f.block(BlockType::Empty);
    f.ins()
        .local_get(e)
        .struct_get(exc, 1)
        .local_tee(arg)
        .ref_is_null();
    f.br_if(no_text);
    f.ins().local_get(arg).ref_as_non_null();
    str.truth(&mut f);
    f.ins().i32_eqz();
    f.br_if(no_text);
    f.ins().local_get(line).ref_as_non_null();
    str.literal(m, &mut f, ": ");
    str.concat(&mut f);
    f.ins().local_get(arg).ref_as_non_null();
    str.concat(&mut f);
    f.ins().local_set(line);
    f.end();
    f.ins().local_get(line).ref_as_non_null();
    str.literal(m, &mut f, "\n");
    str.concat(&mut f);
    f.ins().global_set(raised);
    m.add_func(ty, f.finish())
}

/// The global that holds `class`, which `_start` builds before the
/// program runs, after the global of its base.
fn class_global(
    m: &mut ModuleState,
    str: &mut StrRepr,
    class_type: TypeIdx,
    classes: &mut HashMap<Class, GlobalIdx>,
    class: Class,
) -> GlobalIdx {
    if let Some(global) = classes.get(&class) {
        return *global;
    }
    let base = class
        .base()
        .map(|base| class_global(m, str, class_type, classes, base));
    let name = str.literal_global(m, class.name());
    let global = m.global(
        class_type.ref_to(true),
        &ConstExpr::ref_null(class_type.heap_type()),
    );
    let mut ins = Ins::new(&mut m.init);
    ins.global_get(name).ref_as_non_null();
    match base {
        Some(base) => ins.global_get(base),
        None => ins.ref_null(class_type.heap_type()),
    };
    ins.struct_new(class_type).global_set(global);
    classes.insert(class, global);
    global
}

/// `(func (param i32) (result (ref $Class)))`, which gives the class of
/// the `OSError` of the error of WASI.
fn add_os_error_class(
    m: &mut ModuleState,
    str: &mut StrRepr,
    class_type: TypeIdx,
    classes: &mut HashMap<Class, GlobalIdx>,
) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[class_type.ref_to(false)]);
    let mut f = FuncState::new(1);
    let errno = f.param(0);
    for (code, class, _) in OS_ERRORS {
        if class == Class::OSError {
            continue;
        }
        let global = class_global(m, str, class_type, classes, class);
        f.ins().local_get(errno).i32_const(code).i32_eq();
        f.if_(BlockType::Empty);
        f.ins().global_get(global).ref_as_non_null().return_();
        f.end();
    }
    let global = class_global(m, str, class_type, classes, Class::OSError);
    f.ins().global_get(global).ref_as_non_null();
    m.add_func(ty, f.finish())
}

/// `(func (param i32 i32) (result (ref $Str)))`, which gives the last line
/// of the traceback of the `OSError` of the error of WASI when the second
/// parameter is not 0, and the text of the argument of the `OSError`
/// otherwise. The lines are bytes in one segment, which costs less than a
/// literal for each.
fn add_os_error_line(m: &mut ModuleState, str: &StrRepr) -> FuncIdx {
    let mut texts = Vec::new();
    let mut places = Vec::new();
    for (errno, class, text) in OS_ERRORS {
        let line = last_line(class, text);
        let start = i32::try_from(texts.len()).expect("texts shorter than 2^31");
        let len = i32::try_from(line.len()).expect("a text shorter than 2^31");
        let text_len = i32::try_from(text.len()).expect("a text shorter than 2^31");
        texts.extend_from_slice(line.as_bytes());
        places.push((errno, start, len, text_len));
    }
    let data = m.passive_data(texts);
    let ty = m.func_type(&[ValType::I32, ValType::I32], &[str.val_type(false)]);
    let mut f = FuncState::new(2);
    let (errno, whole) = (f.param(0), f.param(1));
    let start = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    let text_len = f.local(ValType::I32);
    let found = f.block(BlockType::Empty);
    for (code, at, size, text_size) in places {
        f.ins().local_get(errno).i32_const(code).i32_eq();
        f.if_(BlockType::Empty);
        f.ins()
            .i32_const(at)
            .local_set(start)
            .i32_const(size)
            .local_set(len)
            .i32_const(text_size)
            .local_set(text_len);
        f.br(found);
        f.end();
    }
    // wasmtime gives `fd_write` no other error, and neither does the deno
    // of the tests.
    f.ins().unreachable();
    f.end();
    // The text of the argument is the end of the line.
    f.ins().local_get(whole).i32_eqz();
    f.if_(BlockType::Empty);
    f.ins()
        .local_get(start)
        .local_get(len)
        .local_get(text_len)
        .i32_sub()
        .i32_add()
        .local_set(start)
        .local_get(text_len)
        .local_set(len);
    f.end();
    f.ins()
        .local_get(start)
        .local_get(len)
        .array_new_data(str.bytes_type(), data);
    str.ascii(&mut f);
    m.add_func(ty, f.finish())
}

/// `(func (param i32))`, which flushes the standard output and ends the
/// program with the exit code. When the flush fails, the program writes
/// the error to the standard error and ends with the code of CPython.
fn add_exit(
    m: &mut ModuleState,
    host: &Host,
    str: &mut StrRepr,
    os_error_line: FuncIdx,
) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let mut f = FuncState::new(1);
    let code = f.param(0);
    let error = f.local(ValType::I32);
    f.ins().call(host.flush).local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().call(host.to_stderr);
    str.literal(m, &mut f, FLUSH_FAILED);
    str.write(&mut f);
    f.ins().local_get(error).i32_const(1).call(os_error_line);
    str.write(&mut f);
    f.ins()
        .i32_const('\n' as i32)
        .call(host.write_char)
        .i32_const(FLUSH_FAILED_EXIT)
        .local_set(code);
    f.end();
    f.ins().local_get(code).call(host.exit).unreachable();
    m.add_func(ty, f.finish())
}
