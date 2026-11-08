//! The exceptions, and the traceback of an exception that nothing catches.

use wasm_encoder::{BlockType, ConstExpr, HeapType, RefType, StorageType, ValType};

use crate::cpython::{
    FLUSH_FAILED, FLUSH_FAILED_EXIT, MORE_TIME, MORE_TIMES, OS_ERRORS, RECURSION_LIMIT,
    RECURSIVE_CUTOFF, REPEATED, TRACEBACK,
};
use crate::emit::module::{FuncIdx, FuncState, GlobalIdx, ModuleState, TagIdx, TypeIdx};
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
    /// `(array (mut (ref null $Str)))`, with the text of the frame of each
    /// site.
    sites: TypeIdx,
    /// The global that holds the array of the sites.
    sites_global: GlobalIdx,
    /// `(array (mut i32))`, with the id of the line of the frame of each
    /// site.
    lines: TypeIdx,
    /// The global that holds the array of the lines.
    lines_global: GlobalIdx,
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
            (StorageType::Val(ref_to(frame, true)), false),
        ]);
        let exc = m.struct_type(&[
            (StorageType::Val(ref_to(str_type, false)), false),
            (StorageType::Val(ref_to(frame, true)), true),
        ]);
        let tag_type = m.func_type(&[ref_to(exc, false)], &[]);
        let tag = m.tag(tag_type);
        let sites = m.array_type(StorageType::Val(ref_to(str_type, true)), true);
        let sites_global = m.global(
            ref_to(sites, true),
            &ConstExpr::ref_null(HeapType::Concrete(sites.0)),
        );
        let lines = m.array_type(StorageType::Val(ValType::I32), true);
        let lines_global = m.global(
            ref_to(lines, true),
            &ConstExpr::ref_null(HeapType::Concrete(lines.0)),
        );
        let depth = m.global(ValType::I32, &ConstExpr::i32_const(1));
        let repeated = add_repeated(m, str);

        let raise_type = m.func_type(&[ref_to(str_type, false)], &[]);
        let mut f = FuncState::new(1);
        f.ins()
            .local_get(0)
            .ref_null(HeapType::Concrete(frame.0))
            .struct_new(exc.0)
            .throw(tag.0);
        let raise = m.add_func(raise_type, f.finish());
        let os_error_message = add_os_error_message(m, str);
        let os_error_type = m.func_type(&[ValType::I32], &[]);
        let mut f = FuncState::new(1);
        f.ins()
            .local_get(0)
            .call(os_error_message.0)
            .call(raise.0)
            .unreachable();
        let os_error = m.add_func(os_error_type, f.finish());
        let exit = add_exit(m, host, str, os_error_message);

        let frame_type = m.func_type(&[ref_to(exc, false), ValType::I32], &[]);
        let (e, site) = (0, 1);
        // The frame of the site goes before the frames of the functions
        // that the code at the site called.
        let mut f = FuncState::new(2);
        f.ins()
            .local_get(e)
            .local_get(site)
            .local_get(e)
            .struct_get(exc.0, 1)
            .struct_new(frame.0)
            .struct_set(exc.0, 1);
        let add_frame = m.add_func(frame_type, f.finish());

        let mut f = FuncState::new(2);
        f.ins()
            .global_get(depth.0)
            .i32_const(1)
            .i32_sub()
            .global_set(depth.0)
            .local_get(e)
            .local_get(site)
            .call(add_frame.0)
            .local_get(e)
            .throw(tag.0);
        let reraise = m.add_func(frame_type, f.finish());

        // As `StackSummary.format` of CPython, a frame on the line of the
        // frame before it adds to the count of the run, and the traceback
        // writes the count at the end of the run.
        let mut f = FuncState::new(2);
        let next = f.local(ref_to(frame, true));
        let last = f.local(ValType::I32);
        let count = f.local(ValType::I32);
        let line = f.local(ValType::I32);
        f.ins()
            .local_get(e)
            .local_get(site)
            .call(add_frame.0)
            .call(host.flush.0)
            .drop()
            .call(host.to_stderr.0);
        str.literal(m, &mut f, TRACEBACK);
        str.write(&mut f);
        f.ins()
            .local_get(e)
            .struct_get(exc.0, 1)
            .local_set(next)
            .i32_const(-1)
            .local_set(last);
        let done = f.block(BlockType::Empty);
        let frames = f.loop_(BlockType::Empty);
        f.ins().local_get(next).ref_is_null();
        f.br_if(done);
        f.ins()
            .global_get(lines_global.0)
            .local_get(next)
            .struct_get(frame.0, 0)
            .array_get(lines.0)
            .local_tee(line)
            .local_get(last)
            .i32_ne();
        f.if_(BlockType::Empty);
        f.ins()
            .local_get(count)
            .call(repeated.0)
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
        f.ins()
            .global_get(sites_global.0)
            .local_get(next)
            .struct_get(frame.0, 0)
            .array_get(sites.0)
            .ref_as_non_null();
        str.write(&mut f);
        f.end();
        f.ins()
            .local_get(next)
            .struct_get(frame.0, 1)
            .local_set(next);
        f.br(frames);
        f.end();
        f.end();
        f.ins()
            .local_get(count)
            .call(repeated.0)
            .local_get(e)
            .struct_get(exc.0, 0);
        str.write(&mut f);
        f.ins()
            .i32_const('\n' as i32)
            .call(host.write_char.0)
            .i32_const(1)
            .call(exit.0);
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
            lines,
            lines_global,
            depth,
        }
    }

    /// Pushes 1 when a call has room for one more frame, and 0 when the call
    /// raises `RecursionError`.
    pub fn call_fits(&self, f: &mut FuncState) {
        f.ins()
            .global_get(self.depth.0)
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
            .global_get(self.depth.0)
            .i32_const(delta)
            .i32_add()
            .global_set(self.depth.0);
    }

    /// Pops the message and raises an exception with it.
    pub fn raise(&self, f: &mut FuncState) {
        f.ins().call(self.raise.0).unreachable();
    }

    /// Pops an error of WASI and raises its `OSError`.
    pub fn raise_os_error(&self, f: &mut FuncState) {
        f.ins().call(self.os_error.0).unreachable();
    }

    /// Opens the code of the module or of a function, where the handler
    /// that [`Self::end_module`] or [`Self::end_function`] adds catches an
    /// exception.
    pub fn begin(&self, f: &mut FuncState) {
        let caught = f.block(BlockType::Result(ref_to(self.exc, false)));
        f.try_table(BlockType::Empty, self.tag, caught);
    }

    /// Closes what [`Self::begin`] opened in the module, where an exception
    /// that nothing catches ends the program. `site` is the local that holds
    /// the site of the instruction that may raise. As in CPython, the
    /// standard output gets a flush that ignores an error at the end of the
    /// module, and the exit flushes it again.
    pub fn end_module(&self, f: &mut FuncState, site: u32) {
        f.end();
        f.ins()
            .call(self.flush.0)
            .drop()
            .i32_const(0)
            .call(self.exit.0)
            .unreachable();
        f.end();
        f.ins().local_get(site).call(self.uncaught.0);
    }

    /// Closes what [`Self::begin`] opened in a function, where an exception
    /// goes on to the caller with the frame of the function. Every path of
    /// the code of a function ends in a `return`.
    pub fn end_function(&self, f: &mut FuncState, site: u32) {
        f.end();
        f.ins().unreachable();
        f.end();
        f.ins().local_get(site).call(self.reraise.0).unreachable();
    }

    /// Fills the arrays of the sites, with the text and the id of the line
    /// of the frame of each.
    pub fn finish(&self, m: &mut ModuleState, str: &mut StrRepr, sites: &[SiteFrame]) {
        let globals: Vec<GlobalIdx> = sites
            .iter()
            .map(|site| str.literal_global(m, &site.text))
            .collect();
        let len = i32::try_from(sites.len()).expect("fewer than 2^31 sites");
        let mut ins = wasm_encoder::InstructionSink::new(&mut m.init);
        ins.i32_const(len)
            .array_new_default(self.sites.0)
            .global_set(self.sites_global.0)
            .i32_const(len)
            .array_new_default(self.lines.0)
            .global_set(self.lines_global.0);
        for (i, (global, site)) in globals.into_iter().zip(sites).enumerate() {
            let i = i32::try_from(i).expect("fewer than 2^31 sites");
            let line = i32::try_from(site.line).expect("fewer than 2^31 lines");
            ins.global_get(self.sites_global.0)
                .i32_const(i)
                .global_get(global.0)
                .array_set(self.sites.0)
                .global_get(self.lines_global.0)
                .i32_const(i)
                .i32_const(line)
                .array_set(self.lines.0);
        }
    }
}

/// `(func (param i32))`, which writes how many frames of a run the
/// traceback leaves out, when the run has more than the cutoff.
fn add_repeated(m: &mut ModuleState, str: &mut StrRepr) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let count = 0;
    let mut f = FuncState::new(1);
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

fn ref_to(index: TypeIdx, nullable: bool) -> ValType {
    ValType::Ref(RefType {
        nullable,
        heap_type: HeapType::Concrete(index.0),
    })
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
    let errno = 0;
    let mut f = FuncState::new(1);
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
        .array_new_data(str.bytes_type().0, data.0);
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
    let code = 0;
    let mut f = FuncState::new(1);
    let error = f.local(ValType::I32);
    f.ins().call(host.flush.0).local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().call(host.to_stderr.0);
    str.literal(m, &mut f, FLUSH_FAILED);
    str.write(&mut f);
    f.ins().local_get(error).call(os_error_message.0);
    str.write(&mut f);
    f.ins()
        .i32_const('\n' as i32)
        .call(host.write_char.0)
        .i32_const(FLUSH_FAILED_EXIT)
        .local_set(code);
    f.end();
    f.ins().local_get(code).call(host.exit.0).unreachable();
    m.add_func(ty, f.finish())
}
