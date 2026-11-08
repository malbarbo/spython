//! The exceptions, and the traceback of an exception that nothing catches.

use wasm_encoder::{BlockType, Catch, ConstExpr, HeapType, RefType, StorageType, ValType};

use super::str::StrRepr;
use crate::cpython::{
    MORE_TIME, RECURSION_LIMIT, RECURSIVE_CUTOFF, REPEATED, REPEATED_END, TRACEBACK,
};
use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};
use crate::ir::SiteFrame;

pub(super) struct Exceptions {
    /// `(struct (field $message (ref $Str)) (field $tb (mut (ref null $Frame))))`.
    /// The message is the last line of the traceback.
    exc: u32,
    /// The tag of every exception, whose payload is a `(ref $Exc)`.
    tag: u32,
    /// `(func (param (ref $Str)))`, which raises an exception with the
    /// message.
    raise: u32,
    /// `(func (param (ref $Exc) i32))`, which adds the frame of the site to
    /// the traceback, writes the traceback to the standard error, and ends
    /// the program with 1.
    uncaught: u32,
    /// `(func (param (ref $Exc) i32))`, which adds the frame of the site to
    /// the traceback and raises the exception again.
    reraise: u32,
    /// `(array (mut (ref null $Str)))`, with the text of the frame of each
    /// site.
    sites: u32,
    /// The global that holds the array of the sites.
    sites_global: u32,
    /// `(array (mut i32))`, with the line of the frame of each site.
    lines: u32,
    /// The global that holds the array of the lines.
    lines_global: u32,
    /// The global with the number of frames, which starts at 1 for the
    /// frame of the module.
    depth: u32,
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
            &ConstExpr::ref_null(HeapType::Concrete(sites)),
        );
        let lines = m.array_type(StorageType::Val(ValType::I32), true);
        let lines_global = m.global(
            ref_to(lines, true),
            &ConstExpr::ref_null(HeapType::Concrete(lines)),
        );
        let depth = m.global(ValType::I32, &ConstExpr::i32_const(1));
        let repeated = add_repeated(m, str);

        let raise_type = m.func_type(&[ref_to(str_type, false)], &[]);
        let mut f = FuncState::new(1);
        f.ins()
            .local_get(0)
            .ref_null(HeapType::Concrete(frame))
            .struct_new(exc)
            .throw(tag);
        let raise = m.add_func(raise_type, f.finish());

        let frame_type = m.func_type(&[ref_to(exc, false), ValType::I32], &[]);
        let (e, site) = (0, 1);
        // The frame of the site goes before the frames of the functions
        // that the code at the site called.
        let mut f = FuncState::new(2);
        f.ins()
            .local_get(e)
            .local_get(site)
            .local_get(e)
            .struct_get(exc, 1)
            .struct_new(frame)
            .struct_set(exc, 1);
        let add_frame = m.add_func(frame_type, f.finish());

        let mut f = FuncState::new(2);
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
        let next = f.local(ref_to(frame, true));
        let last = f.local(ValType::I32);
        let count = f.local(ValType::I32);
        let line = f.local(ValType::I32);
        f.ins()
            .local_get(e)
            .local_get(site)
            .call(add_frame)
            .call(host.to_stderr);
        str.literal(m, &mut f, TRACEBACK);
        str.write(&mut f);
        f.ins()
            .local_get(e)
            .struct_get(exc, 1)
            .local_set(next)
            .i32_const(-1)
            .local_set(last)
            .block(BlockType::Empty)
            .loop_(BlockType::Empty)
            .local_get(next)
            .ref_is_null()
            .br_if(1)
            .global_get(lines_global)
            .local_get(next)
            .struct_get(frame, 0)
            .array_get(lines)
            .local_tee(line)
            .local_get(last)
            .i32_ne()
            .if_(BlockType::Empty)
            .local_get(count)
            .call(repeated)
            .local_get(line)
            .local_set(last)
            .i32_const(0)
            .local_set(count)
            .end()
            .local_get(count)
            .i32_const(1)
            .i32_add()
            .local_tee(count)
            .i32_const(RECURSIVE_CUTOFF)
            .i32_le_s()
            .if_(BlockType::Empty)
            .global_get(sites_global)
            .local_get(next)
            .struct_get(frame, 0)
            .array_get(sites)
            .ref_as_non_null();
        str.write(&mut f);
        f.ins()
            .end()
            .local_get(next)
            .struct_get(frame, 1)
            .local_set(next)
            .br(0)
            .end()
            .end()
            .local_get(count)
            .call(repeated)
            .local_get(e)
            .struct_get(exc, 0);
        str.write(&mut f);
        f.ins()
            .i32_const('\n' as i32)
            .call(host.write_char)
            .i32_const(1)
            .call(host.exit);
        let uncaught = m.add_func(frame_type, f.finish());

        Exceptions {
            exc,
            tag,
            raise,
            uncaught,
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

    /// Opens the code of the module or of a function, where the handler
    /// that [`Self::end_module`] or [`Self::end_function`] adds catches an
    /// exception.
    pub fn begin(&self, f: &mut FuncState) {
        f.ins()
            .block(BlockType::Result(ref_to(self.exc, false)))
            .try_table(
                BlockType::Empty,
                [Catch::One {
                    tag: self.tag,
                    label: 0,
                }],
            );
    }

    /// Closes what [`Self::begin`] opened in the module, where an exception
    /// that nothing catches ends the program. `site` is the local that holds
    /// the site of the instruction that may raise.
    pub fn end_module(&self, f: &mut FuncState, site: u32) {
        f.ins()
            .end()
            .return_()
            .end()
            .local_get(site)
            .call(self.uncaught);
    }

    /// Closes what [`Self::begin`] opened in a function, where an exception
    /// goes on to the caller with the frame of the function. Every path of
    /// the code of a function ends in a `return`.
    pub fn end_function(&self, f: &mut FuncState, site: u32) {
        f.ins()
            .end()
            .unreachable()
            .end()
            .local_get(site)
            .call(self.reraise)
            .unreachable();
    }

    /// Fills the arrays of the sites, with the text and the line of the
    /// frame of each.
    pub fn finish(&self, m: &mut ModuleState, str: &mut StrRepr, sites: &[SiteFrame]) {
        let globals: Vec<u32> = sites
            .iter()
            .map(|site| str.literal_global(m, &site.text))
            .collect();
        let len = i32::try_from(sites.len()).expect("fewer than 2^31 sites");
        let mut ins = wasm_encoder::InstructionSink::new(&mut m.init);
        ins.i32_const(len)
            .array_new_default(self.sites)
            .global_set(self.sites_global)
            .i32_const(len)
            .array_new_default(self.lines)
            .global_set(self.lines_global);
        for (i, (global, site)) in globals.into_iter().zip(sites).enumerate() {
            let i = i32::try_from(i).expect("fewer than 2^31 sites");
            let line = i32::try_from(site.line).expect("fewer than 2^31 lines");
            ins.global_get(self.sites_global)
                .i32_const(i)
                .global_get(global)
                .array_set(self.sites)
                .global_get(self.lines_global)
                .i32_const(i)
                .i32_const(line)
                .array_set(self.lines);
        }
    }
}

/// `(func (param i32))`, which writes how many frames of a run the
/// traceback leaves out, when the run has more than the cutoff.
fn add_repeated(m: &mut ModuleState, str: &mut StrRepr) -> u32 {
    let ty = m.func_type(&[ValType::I32], &[]);
    let count = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .local_get(count)
        .i32_const(RECURSIVE_CUTOFF)
        .i32_gt_s()
        .if_(BlockType::Empty)
        .local_get(count)
        .i32_const(RECURSIVE_CUTOFF)
        .i32_sub()
        .local_set(count);
    str.literal(m, &mut f, REPEATED);
    str.write(&mut f);
    f.ins().local_get(count).i64_extend_i32_s();
    str.decimal(&mut f);
    str.write(&mut f);
    str.literal(m, &mut f, MORE_TIME);
    str.write(&mut f);
    f.ins()
        .local_get(count)
        .i32_const(1)
        .i32_gt_s()
        .if_(BlockType::Empty);
    str.literal(m, &mut f, "s");
    str.write(&mut f);
    f.ins().end();
    str.literal(m, &mut f, REPEATED_END);
    str.write(&mut f);
    f.ins().end();
    m.add_func(ty, f.finish())
}

fn ref_to(index: u32, nullable: bool) -> ValType {
    ValType::Ref(RefType {
        nullable,
        heap_type: HeapType::Concrete(index),
    })
}
