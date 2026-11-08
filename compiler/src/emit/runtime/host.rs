//! What the module asks of the WASI host: the standard output and the
//! standard error, written through buffers in linear memory, and the exit.
//!
//! The standard output keeps the two buffers of CPython 3.13, so a write
//! that fails raises at the same `print` as in CPython. The `TextIOWrapper`
//! holds the text of the writes until it has `CHUNK` bytes, and the
//! `BufferedWriter` under it holds what it gets until it has a block. A
//! terminal flushes both at each line.

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, ValType};

use crate::emit::module::{FuncIdx, FuncState, GlobalIdx, ModuleState};

/// Where `fd_write` reads the one iovec of a write.
const IOVEC: u64 = 0;
/// Where `fd_write` stores the number of bytes it wrote.
const NWRITTEN: u64 = 8;
/// Where `fd_fdstat_get` stores the 24 bytes of its answer, aligned to 8.
const FDSTAT: i32 = 16;
/// The buffer of the `BufferedWriter`, which holds at most a block.
const BUFFERED: i32 = 64;
/// The text of the `TextIOWrapper` that waits for `buffer.write`, followed
/// by the text of the write that is going on. Before a write it holds less
/// than `CHUNK` bytes, and a write with `CHUNK` bytes goes on without it.
const PENDING: i32 = 8192;
/// The `_CHUNK_SIZE` of the `TextIOWrapper`.
const CHUNK: i32 = 8192;
/// The buffer of the standard error, up to the end of the memory.
const STDERR_BUFFER: i32 = 32768;
const STDERR_BUFFER_END: i32 = 65536;
/// The longest UTF-8 encoding of a code point.
const MAX_CHAR_BYTES: i32 = 4;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;
/// The file type that WASI gives a terminal.
const CHARACTER_DEVICE: i32 = 2;
/// The `st_blksize` of Linux for a terminal, and for a pipe or a file.
/// WASI does not give it, and another file system may have another one.
const TERMINAL_BLOCK: i32 = 1024;
const BLOCK: i32 = 4096;

/// The UTF-8 bytes of each length, as (shift, mark) for the lead byte and
/// the continuation bytes after it.
const ENCODINGS: [(i32, &[(i32, i32)]); 4] = [
    (0x80, &[(0, 0x00)]),
    (0x800, &[(6, 0xC0), (0, 0x80)]),
    (0x10000, &[(12, 0xE0), (6, 0x80), (0, 0x80)]),
    (0x110000, &[(18, 0xF0), (12, 0x80), (6, 0x80), (0, 0x80)]),
];

pub(crate) struct Host {
    /// `(func)`, which the program calls before anything else.
    pub start: FuncIdx,
    /// `(func (param i32))`, which adds a code point to the write that is
    /// going on.
    pub write_char: FuncIdx,
    /// `(func)`, which ends a write, as one call of `write` in CPython.
    pub end_write: FuncIdx,
    /// The global with the error of WASI of the write that failed, and 0
    /// when none did since the program last read it. A write after a
    /// failure does nothing until the program reads it.
    pub error: GlobalIdx,
    /// `(func (result i32))`, which flushes the standard output and returns
    /// the error of WASI, or 0.
    pub flush: FuncIdx,
    /// `(func)`, which sends what comes after it to the standard error.
    pub to_stderr: FuncIdx,
    /// `(func (param i32))`, which flushes the standard error and ends the
    /// program with the exit code.
    pub exit: FuncIdx,
}

/// The globals of the buffers.
struct State {
    error: GlobalIdx,
    /// 1 when the standard output is a terminal, and 0 otherwise.
    terminal: GlobalIdx,
    /// The size of the buffer of the `BufferedWriter`.
    block: GlobalIdx,
    /// The bytes of `BUFFERED` from `written` to `buffered` wait for the
    /// host. `written` passes 0 when a flush fails after a partial write.
    written: GlobalIdx,
    buffered: GlobalIdx,
    /// The bytes of `PENDING` before the write that is going on.
    pending: GlobalIdx,
    /// The bytes of `PENDING`, with the ones of the write that is going on.
    end: GlobalIdx,
    /// 1 when the write that is going on has `CHUNK` bytes, and goes to the
    /// host as `PENDING` fills.
    direct: GlobalIdx,
    /// 1 when the write that is going on has a `\n` or a `\r`.
    newline: GlobalIdx,
    /// 1 after [`Host::to_stderr`].
    to_stderr: GlobalIdx,
    /// The bytes of `STDERR_BUFFER`.
    stderr_end: GlobalIdx,
}

/// The functions that the others call.
struct Funcs {
    write_all: FuncIdx,
    flush_buffered: FuncIdx,
    write_buffered: FuncIdx,
    flush_pending: FuncIdx,
    fail: FuncIdx,
    flush_stderr: FuncIdx,
}

impl Host {
    pub fn new(m: &mut ModuleState) -> Self {
        let fd_write_ty = m.func_type(
            &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I32],
        );
        let fd_write = m.import_func("wasi_snapshot_preview1", "fd_write", fd_write_ty);
        let fdstat_ty = m.func_type(&[ValType::I32, ValType::I32], &[ValType::I32]);
        let fd_fdstat_get = m.import_func("wasi_snapshot_preview1", "fd_fdstat_get", fdstat_ty);
        let proc_exit_ty = m.func_type(&[ValType::I32], &[]);
        let proc_exit = m.import_func("wasi_snapshot_preview1", "proc_exit", proc_exit_ty);
        // WASI reads the buffers from the memory the module exports by this name.
        m.memory("memory");
        let mut global = |init| m.global(ValType::I32, &ConstExpr::i32_const(init));
        let s = State {
            error: global(0),
            terminal: global(0),
            block: global(BLOCK),
            written: global(0),
            buffered: global(0),
            pending: global(0),
            end: global(0),
            direct: global(0),
            newline: global(0),
            to_stderr: global(0),
            stderr_end: global(0),
        };
        let write_once = add_write_once(m, fd_write);
        let write_all = add_write_all(m, write_once);
        let flush_buffered = add_flush_buffered(m, &s, write_once);
        let write_buffered = add_write_buffered(m, &s, write_once, flush_buffered);
        let flush_pending = add_flush_pending(m, &s, write_buffered);
        let fail = add_fail(m, &s);
        let flush_stderr = add_flush_stderr(m, &s, write_all);
        let funcs = Funcs {
            write_all,
            flush_buffered,
            write_buffered,
            flush_pending,
            fail,
            flush_stderr,
        };
        Host {
            start: add_start(m, &s, fd_fdstat_get),
            write_char: add_write_char(m, &s, &funcs),
            end_write: add_end_write(m, &s, &funcs),
            error: s.error,
            flush: add_flush(m, &funcs),
            to_stderr: add_to_stderr(m, &s),
            exit: add_exit(m, &funcs, proc_exit),
        }
    }
}

fn mem(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }
}

/// Adds `delta`, which the code before it pushed, to `global`.
fn add_to(f: &mut FuncState, global: GlobalIdx) {
    f.ins().global_get(global.0).i32_add().global_set(global.0);
}

/// `(func)`, which asks WASI whether the standard output is a terminal, as
/// CPython does for its buffers.
fn add_start(m: &mut ModuleState, s: &State, fd_fdstat_get: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    f.ins()
        .i32_const(STDOUT)
        .i32_const(FDSTAT)
        .call(fd_fdstat_get.0)
        .i32_eqz()
        .i32_const(FDSTAT)
        .i32_load8_u(mem(0, 0))
        .i32_const(CHARACTER_DEVICE)
        .i32_eq()
        .i32_and()
        .global_set(s.terminal.0)
        .global_get(s.terminal.0);
    f.if_(BlockType::Result(ValType::I32));
    f.ins().i32_const(TERMINAL_BLOCK);
    f.else_();
    f.ins().i32_const(BLOCK);
    f.end();
    f.ins().global_set(s.block.0);
    m.add_func(ty, f.finish())
}

/// `(func (param $fd i32) (param $ptr i32) (param $len i32) (result i32))`,
/// which asks WASI once to write the bytes. It returns how many the host
/// wrote, or the error of WASI as a negative number.
fn add_write_once(m: &mut ModuleState, fd_write: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32; 3], &[ValType::I32]);
    let (fd, ptr, len) = (0, 1, 2);
    let mut f = FuncState::new(3);
    let error = f.local(ValType::I32);
    f.ins()
        .i32_const(0)
        .local_get(ptr)
        .i32_store(mem(IOVEC, 2))
        .i32_const(0)
        .local_get(len)
        .i32_store(mem(IOVEC + 4, 2))
        .local_get(fd)
        .i32_const(IOVEC as i32)
        .i32_const(1)
        .i32_const(NWRITTEN as i32)
        .call(fd_write.0)
        .local_tee(error);
    f.if_(BlockType::Result(ValType::I32));
    f.ins().i32_const(0).local_get(error).i32_sub();
    f.else_();
    f.ins().i32_const(0).i32_load(mem(NWRITTEN, 2));
    f.end();
    m.add_func(ty, f.finish())
}

/// `(func (param $fd i32) (param $ptr i32) (param $len i32) (result i32))`,
/// which asks WASI again until it wrote every byte, and returns the error
/// of WASI, or 0.
fn add_write_all(m: &mut ModuleState, write_once: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32; 3], &[ValType::I32]);
    let (fd, ptr, len) = (0, 1, 2);
    let mut f = FuncState::new(3);
    let n = f.local(ValType::I32);
    let done = f.block(BlockType::Empty);
    let rest = f.loop_(BlockType::Empty);
    f.ins().local_get(len).i32_eqz();
    f.br_if(done);
    f.ins()
        .local_get(fd)
        .local_get(ptr)
        .local_get(len)
        .call(write_once.0)
        .local_tee(n)
        .i32_const(0)
        .i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).local_get(n).i32_sub().return_();
    f.end();
    f.ins()
        .local_get(ptr)
        .local_get(n)
        .i32_add()
        .local_set(ptr)
        .local_get(len)
        .local_get(n)
        .i32_sub()
        .local_set(len);
    f.br(rest);
    f.end();
    f.end();
    f.ins().i32_const(0);
    m.add_func(ty, f.finish())
}

/// `(func (result i32))`, the `flush` of the `BufferedWriter`, which returns
/// the error of WASI, or 0. A failure keeps the bytes that the host did not
/// take.
fn add_flush_buffered(m: &mut ModuleState, s: &State, write_once: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[], &[ValType::I32]);
    let mut f = FuncState::new(0);
    let n = f.local(ValType::I32);
    let done = f.block(BlockType::Empty);
    let rest = f.loop_(BlockType::Empty);
    f.ins()
        .global_get(s.written.0)
        .global_get(s.buffered.0)
        .i32_ge_s();
    f.br_if(done);
    f.ins()
        .i32_const(STDOUT)
        .i32_const(BUFFERED)
        .global_get(s.written.0)
        .i32_add()
        .global_get(s.buffered.0)
        .global_get(s.written.0)
        .i32_sub()
        .call(write_once.0)
        .local_tee(n)
        .i32_const(0)
        .i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).local_get(n).i32_sub().return_();
    f.end();
    f.ins().local_get(n);
    add_to(&mut f, s.written);
    f.br(rest);
    f.end();
    f.end();
    f.ins()
        .i32_const(0)
        .global_set(s.written.0)
        .i32_const(0)
        .global_set(s.buffered.0)
        .i32_const(0);
    m.add_func(ty, f.finish())
}

/// `(func (param $ptr i32) (param $len i32) (result i32))`, the `write` of
/// the `BufferedWriter`, which returns the error of WASI, or 0. Bytes that
/// fit stay in the buffer. Otherwise the buffer goes to the host, and then
/// the bytes themselves, as long as they fill a block.
fn add_write_buffered(
    m: &mut ModuleState,
    s: &State,
    write_once: FuncIdx,
    flush_buffered: FuncIdx,
) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32; 2], &[ValType::I32]);
    let (ptr, len) = (0, 1);
    let mut f = FuncState::new(2);
    let n = f.local(ValType::I32);
    f.ins()
        .local_get(len)
        .global_get(s.block.0)
        .global_get(s.buffered.0)
        .i32_sub()
        .i32_le_s()
        .local_get(len)
        .global_get(s.block.0)
        .i32_lt_s()
        .i32_and();
    f.if_(BlockType::Empty);
    f.ins()
        .i32_const(BUFFERED)
        .global_get(s.buffered.0)
        .i32_add()
        .local_get(ptr)
        .local_get(len)
        .memory_copy(0, 0)
        .local_get(len);
    add_to(&mut f, s.buffered);
    f.ins().i32_const(0).return_();
    f.end();
    f.ins().call(flush_buffered.0).local_tee(n);
    f.if_(BlockType::Empty);
    f.ins().local_get(n).return_();
    f.end();
    let done = f.block(BlockType::Empty);
    let rest = f.loop_(BlockType::Empty);
    f.ins().local_get(len).global_get(s.block.0).i32_lt_s();
    f.br_if(done);
    f.ins()
        .i32_const(STDOUT)
        .local_get(ptr)
        .local_get(len)
        .call(write_once.0)
        .local_tee(n)
        .i32_const(0)
        .i32_lt_s();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).local_get(n).i32_sub().return_();
    f.end();
    f.ins()
        .local_get(ptr)
        .local_get(n)
        .i32_add()
        .local_set(ptr)
        .local_get(len)
        .local_get(n)
        .i32_sub()
        .local_set(len);
    f.br(rest);
    f.end();
    f.end();
    f.ins()
        .i32_const(BUFFERED)
        .local_get(ptr)
        .local_get(len)
        .memory_copy(0, 0)
        .i32_const(0)
        .global_set(s.written.0)
        .local_get(len)
        .global_set(s.buffered.0)
        .i32_const(0);
    m.add_func(ty, f.finish())
}

/// `(func (result i32))`, the `_textiowrapper_writeflush` of CPython, which
/// passes `PENDING` to the `BufferedWriter` and returns the error of WASI,
/// or 0. `PENDING` ends empty even when the write fails.
fn add_flush_pending(m: &mut ModuleState, s: &State, write_buffered: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[], &[ValType::I32]);
    let mut f = FuncState::new(0);
    let len = f.local(ValType::I32);
    f.ins()
        .global_get(s.end.0)
        .local_set(len)
        .i32_const(0)
        .global_set(s.end.0)
        .i32_const(0)
        .global_set(s.pending.0)
        .local_get(len)
        .i32_eqz();
    f.if_(BlockType::Empty);
    f.ins().i32_const(0).return_();
    f.end();
    f.ins()
        .i32_const(PENDING)
        .local_get(len)
        .call(write_buffered.0);
    m.add_func(ty, f.finish())
}

/// `(func (param i32))`, which keeps the error of WASI and drops the write
/// that is going on, as CPython drops the text of a write that fails.
fn add_fail(m: &mut ModuleState, s: &State) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let error = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .local_get(error)
        .global_set(s.error.0)
        .i32_const(0)
        .global_set(s.end.0)
        .i32_const(0)
        .global_set(s.pending.0)
        .i32_const(0)
        .global_set(s.direct.0);
    m.add_func(ty, f.finish())
}

/// `(func)`, which sends `STDERR_BUFFER` to the host. CPython ignores an
/// error of the standard error.
fn add_flush_stderr(m: &mut ModuleState, s: &State, write_all: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    f.ins()
        .i32_const(STDERR)
        .i32_const(STDERR_BUFFER)
        .global_get(s.stderr_end.0)
        .call(write_all.0)
        .drop()
        .i32_const(0)
        .global_set(s.stderr_end.0);
    m.add_func(ty, f.finish())
}

/// Stores the UTF-8 bytes of the code point in `c` at the address in
/// `addr`, and sets `n` to their number.
fn encode(f: &mut FuncState, c: u32, addr: u32, n: u32) {
    let done = f.block(BlockType::Empty);
    for (limit, bytes) in ENCODINGS {
        f.ins().local_get(c).i32_const(limit).i32_lt_u();
        f.if_(BlockType::Empty);
        for (offset, (shift, mark)) in bytes.iter().enumerate() {
            let mut ins = f.ins();
            ins.local_get(addr).local_get(c);
            if *shift > 0 {
                ins.i32_const(*shift).i32_shr_u();
            }
            // The lead byte of a one-byte encoding is the code point itself.
            if *mark != 0 {
                ins.i32_const(0x3F).i32_and().i32_const(*mark).i32_or();
            }
            ins.i32_store8(mem(offset as u64, 0));
        }
        f.ins()
            .i32_const(i32::try_from(bytes.len()).expect("at most 4 bytes"))
            .local_set(n);
        f.br(done);
        f.end();
    }
    f.ins().unreachable();
    f.end();
}

/// The function that adds a code point to the write that is going on.
fn add_write_char(m: &mut ModuleState, s: &State, funcs: &Funcs) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let c = 0;
    let mut f = FuncState::new(1);
    let addr = f.local(ValType::I32);
    let n = f.local(ValType::I32);
    let error = f.local(ValType::I32);

    f.ins().global_get(s.to_stderr.0);
    f.if_(BlockType::Empty);
    f.ins()
        .global_get(s.stderr_end.0)
        .i32_const(STDERR_BUFFER_END - STDERR_BUFFER - MAX_CHAR_BYTES)
        .i32_gt_u();
    f.if_(BlockType::Empty);
    f.ins().call(funcs.flush_stderr.0);
    f.end();
    f.ins()
        .i32_const(STDERR_BUFFER)
        .global_get(s.stderr_end.0)
        .i32_add()
        .local_set(addr);
    encode(&mut f, c, addr, n);
    f.ins().local_get(n);
    add_to(&mut f, s.stderr_end);
    f.ins().return_();
    f.end();

    f.ins().global_get(s.error.0);
    f.if_(BlockType::Empty);
    f.ins().return_();
    f.end();
    f.ins()
        .i32_const(PENDING)
        .global_get(s.end.0)
        .i32_add()
        .local_set(addr);
    encode(&mut f, c, addr, n);
    f.ins().local_get(n);
    add_to(&mut f, s.end);
    f.ins()
        .local_get(c)
        .i32_const('\n' as i32)
        .i32_eq()
        .local_get(c)
        .i32_const('\r' as i32)
        .i32_eq()
        .i32_or();
    f.if_(BlockType::Empty);
    f.ins().i32_const(1).global_set(s.newline.0);
    f.end();

    // A write with `CHUNK` bytes goes to the host as `PENDING` fills.
    f.ins().global_get(s.direct.0);
    f.if_(BlockType::Empty);
    f.ins().global_get(s.end.0).i32_const(2 * CHUNK).i32_ge_u();
    f.if_(BlockType::Empty);
    f.ins()
        .i32_const(STDOUT)
        .i32_const(PENDING)
        .global_get(s.end.0)
        .call(funcs.write_all.0)
        .local_set(error)
        .i32_const(0)
        .global_set(s.end.0)
        .local_get(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0);
    f.end();
    f.end();
    f.ins().return_();
    f.end();

    f.ins()
        .global_get(s.end.0)
        .global_get(s.pending.0)
        .i32_sub()
        .i32_const(CHUNK)
        .i32_lt_u();
    f.if_(BlockType::Empty);
    f.ins().return_();
    f.end();
    // The write has `CHUNK` bytes, so CPython passes the text before it to
    // the `BufferedWriter` on its own. The write comes next, and it is too
    // large for the buffer, so the buffer goes to the host first.
    f.ins().global_get(s.pending.0);
    f.if_(BlockType::Empty);
    f.ins()
        .i32_const(PENDING)
        .global_get(s.pending.0)
        .call(funcs.write_buffered.0)
        .local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0).return_();
    f.end();
    f.ins()
        .i32_const(PENDING)
        .i32_const(PENDING)
        .global_get(s.pending.0)
        .i32_add()
        .global_get(s.end.0)
        .global_get(s.pending.0)
        .i32_sub()
        .memory_copy(0, 0)
        .global_get(s.end.0)
        .global_get(s.pending.0)
        .i32_sub()
        .global_set(s.end.0)
        .i32_const(0)
        .global_set(s.pending.0);
    f.end();
    f.ins().call(funcs.flush_buffered.0).local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0).return_();
    f.end();
    f.ins().i32_const(1).global_set(s.direct.0);
    m.add_func(ty, f.finish())
}

/// The function that ends a write, as the end of `TextIOWrapper.write`.
fn add_end_write(m: &mut ModuleState, s: &State, funcs: &Funcs) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    let error = f.local(ValType::I32);
    let done = f.block(BlockType::Empty);
    f.ins()
        .global_get(s.to_stderr.0)
        .global_get(s.error.0)
        .i32_or();
    f.br_if(done);

    f.ins().global_get(s.direct.0);
    f.if_(BlockType::Empty);
    f.ins()
        .i32_const(STDOUT)
        .i32_const(PENDING)
        .global_get(s.end.0)
        .call(funcs.write_all.0)
        .local_set(error)
        .i32_const(0)
        .global_set(s.end.0)
        .i32_const(0)
        .global_set(s.direct.0)
        .local_get(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0);
    f.end();
    f.br(done);
    f.end();

    // A terminal flushes both buffers at each line.
    f.ins()
        .global_get(s.end.0)
        .i32_const(CHUNK)
        .i32_ge_u()
        .global_get(s.terminal.0)
        .global_get(s.newline.0)
        .i32_and()
        .i32_or();
    f.if_(BlockType::Empty);
    f.ins().call(funcs.flush_pending.0).local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0);
    f.br(done);
    f.end();
    f.ins()
        .global_get(s.terminal.0)
        .global_get(s.newline.0)
        .i32_and();
    f.if_(BlockType::Empty);
    f.ins().call(funcs.flush_buffered.0).local_tee(error);
    f.if_(BlockType::Empty);
    f.ins().local_get(error).call(funcs.fail.0);
    f.end();
    f.end();
    f.br(done);
    f.end();
    f.ins().global_get(s.end.0).global_set(s.pending.0);
    f.end();
    f.ins().i32_const(0).global_set(s.newline.0);
    m.add_func(ty, f.finish())
}

/// The `flush` of `sys.stdout`, which passes `PENDING` to the
/// `BufferedWriter` and then flushes it.
fn add_flush(m: &mut ModuleState, funcs: &Funcs) -> FuncIdx {
    let ty = m.func_type(&[], &[ValType::I32]);
    let mut f = FuncState::new(0);
    let error = f.local(ValType::I32);
    f.ins()
        .call(funcs.flush_pending.0)
        .local_tee(error)
        .i32_eqz();
    f.if_(BlockType::Empty);
    f.ins().call(funcs.flush_buffered.0).local_set(error);
    f.end();
    f.ins().local_get(error);
    m.add_func(ty, f.finish())
}

fn add_to_stderr(m: &mut ModuleState, s: &State) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    f.ins().i32_const(1).global_set(s.to_stderr.0);
    m.add_func(ty, f.finish())
}

fn add_exit(m: &mut ModuleState, funcs: &Funcs, proc_exit: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let code = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .call(funcs.flush_stderr.0)
        .local_get(code)
        .call(proc_exit.0)
        .unreachable();
    m.add_func(ty, f.finish())
}
