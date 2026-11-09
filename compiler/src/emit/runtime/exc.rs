//! The exceptions, and the traceback of an exception that nothing catches.

use std::collections::HashMap;

use wasm_encoder::{BlockType, ConstExpr, StorageType, ValType};

use crate::cpython::{
    FLUSH_FAILED, FLUSH_FAILED_EXIT, MORE_TIME, MORE_TIMES, OS_ERRORS, RECURSION_LIMIT,
    RECURSIVE_CUTOFF, REPEATED, TRACEBACK,
};
use crate::emit::module::{
    FuncIdx, FuncState, GlobalIdx, Ins, LocalIdx, ModuleState, TagIdx, TypeIdx,
};
use crate::emit::repr::str::StrRepr;
use crate::emit::runtime::host::Host;
use crate::ir::SiteFrame;

pub(in crate::emit) struct Exceptions {
    /// `(struct (field $message (ref $Str)) (field $tb (mut (ref null $Frame))))`.
    /// The message is the last line of the traceback.
    exc: TypeIdx,
    /// The tag of every exception, whose payload is a `(ref $Exc)`.
    tag: TagIdx,
    /// `(func (param (ref $Str)))`, which raises an exception with the
    /// message.
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
}

impl Exceptions {
    pub fn new(m: &mut ModuleState, host: &Host, str: &mut StrRepr) -> Self {
        let str_type = str.type_index();
        // `(struct (field $site i32) (field $next (ref null $Frame)))`, a
        // frame of the traceback, from the outermost one.
        let frame = m.next_type_index();
        m.struct_type(&[
            (StorageType::Val(ValType::I32), false),
            (StorageType::Val(frame.ref_to(true)), false),
        ]);
        let exc = m.struct_type(&[
            (StorageType::Val(str_type.ref_to(false)), false),
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

        let raise_type = m.func_type(&[str_type.ref_to(false)], &[]);
        let mut f = FuncState::new(1);
        let message = f.param(0);
        f.ins()
            .local_get(message)
            .ref_null(frame.heap_type())
            .struct_new(exc)
            .throw(tag);
        let raise = m.add_func(raise_type, f.finish());
        let os_error_message = add_os_error_message(m, str);
        let os_error_type = m.func_type(&[ValType::I32], &[]);
        let mut f = FuncState::new(1);
        let errno = f.param(0);
        f.ins()
            .local_get(errno)
            .call(os_error_message)
            .call(raise)
            .unreachable();
        let os_error = m.add_func(os_error_type, f.finish());
        let exit = add_exit(m, host, str, os_error_message);

        let frame_type = m.func_type(&[exc.ref_to(false), ValType::I32], &[]);
        // The frame of the site goes before the frames of the functions
        // that the code at the site called.
        let mut f = FuncState::new(2);
        let (e, site) = (f.param(0), f.param(1));
        f.ins()
            .local_get(e)
            .local_get(site)
            .local_get(e)
            .struct_get(exc, 1)
            .struct_new(frame)
            .struct_set(exc, 1);
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
            .struct_get(exc, 1)
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
            .struct_get(exc, 0);
        str.write(&mut f);
        f.ins()
            .i32_const('\n' as i32)
            .call(host.write_char)
            .i32_const(1)
            .call(exit);
        let uncaught = m.add_func(frame_type, f.finish());

        Exceptions {
            exc,
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
        }
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

    /// Pops the message and raises an exception with it.
    pub fn raise(&self, f: &mut FuncState) {
        f.ins().call(self.raise).unreachable();
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

/// The function that raises the `OSError` of an error of WASI. The texts
/// are bytes in one segment, which costs less than a literal for each.
/// `(func (param i32) (result (ref $Str)))`, which gives the last line of
/// the traceback of the `OSError` of the error of WASI.
fn add_os_error_message(m: &mut ModuleState, str: &StrRepr) -> FuncIdx {
    let mut texts = Vec::new();
    let mut places = Vec::new();
    for (errno, text) in OS_ERRORS {
        let start = i32::try_from(texts.len()).expect("texts shorter than 2^31");
        let len = i32::try_from(text.len()).expect("a text shorter than 2^31");
        texts.extend_from_slice(text.as_bytes());
        places.push((errno, start, len));
    }
    let data = m.passive_data(texts);
    let ty = m.func_type(&[ValType::I32], &[str.val_type(false)]);
    let mut f = FuncState::new(1);
    let errno = f.param(0);
    let start = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    let found = f.block(BlockType::Empty);
    for (code, at, size) in places {
        f.ins().local_get(errno).i32_const(code).i32_eq();
        f.if_(BlockType::Empty);
        f.ins()
            .i32_const(at)
            .local_set(start)
            .i32_const(size)
            .local_set(len);
        f.br(found);
        f.end();
    }
    // wasmtime gives `fd_write` no other error, and neither does the deno
    // of the tests.
    f.ins().unreachable();
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
    os_error_message: FuncIdx,
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
    f.ins().local_get(error).call(os_error_message);
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
