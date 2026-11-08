//! What the module asks of the WASI host: the standard output and the
//! standard error, written through a buffer in linear memory, and the exit.

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, ValType};

use crate::emit::module::{FuncIdx, FuncState, GlobalIdx, ModuleState};

/// Where `fd_write` reads the one iovec of a write.
const IOVEC: u64 = 0;
/// Where `fd_write` stores the number of bytes it wrote.
const NWRITTEN: u64 = 8;
const BUFFER_START: i32 = 16;
const BUFFER_END: i32 = 65536;
/// The longest UTF-8 encoding of a code point.
const MAX_CHAR_BYTES: i32 = 4;
const STDOUT: i32 = 1;
const STDERR: i32 = 2;

pub(crate) struct Host {
    /// `(func (param i32))`, which writes a code point to the standard output.
    pub write_char: FuncIdx,
    /// `(func)`, which sends what the buffer holds to the host.
    pub flush: FuncIdx,
    /// The global with the error of WASI of the last write that failed,
    /// and 0 when none did since the program last read it.
    pub error: GlobalIdx,
    /// `(func)`, which flushes the buffer and sends what comes after it to
    /// the standard error.
    pub to_stderr: FuncIdx,
    /// `(func (param i32))`, which flushes the buffer and ends the program
    /// with the exit code.
    pub exit: FuncIdx,
}

impl Host {
    pub fn new(m: &mut ModuleState) -> Self {
        let fd_write_ty = m.func_type(
            &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I32],
        );
        let fd_write = m.import_func("wasi_snapshot_preview1", "fd_write", fd_write_ty);
        let proc_exit_ty = m.func_type(&[ValType::I32], &[]);
        let proc_exit = m.import_func("wasi_snapshot_preview1", "proc_exit", proc_exit_ty);
        // WASI reads the buffers from the memory the module exports by this name.
        m.memory("memory");
        let pos = m.global(ValType::I32, &ConstExpr::i32_const(BUFFER_START));
        // The file descriptor the buffer goes to.
        let fd = m.global(ValType::I32, &ConstExpr::i32_const(STDOUT));
        let error = m.global(ValType::I32, &ConstExpr::i32_const(0));
        let flush = add_flush(m, fd_write, pos, fd, error);
        let write_char = add_write_char(m, flush, pos);
        let to_stderr = add_to_stderr(m, flush, fd);
        let exit = add_exit(m, flush, proc_exit);
        Host {
            write_char,
            flush,
            error,
            to_stderr,
            exit,
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

fn add_flush(
    m: &mut ModuleState,
    fd_write: FuncIdx,
    pos: GlobalIdx,
    fd: GlobalIdx,
    error: GlobalIdx,
) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    let ptr = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    let result = f.local(ValType::I32);
    f.ins()
        .i32_const(BUFFER_START)
        .local_set(ptr)
        .global_get(pos.0)
        .i32_const(BUFFER_START)
        .i32_sub()
        .local_set(len);
    // A write can take fewer bytes than it was given, so the loop asks
    // again for the rest. An error drops the rest, as CPython drops what
    // it fails to write, and waits in `error` for the code that raises it.
    let done = f.block(BlockType::Empty);
    let rest = f.loop_(BlockType::Empty);
    f.ins().local_get(len).i32_eqz();
    f.br_if(done);
    f.ins()
        .i32_const(0)
        .local_get(ptr)
        .i32_store(mem(IOVEC, 2))
        .i32_const(0)
        .local_get(len)
        .i32_store(mem(IOVEC + 4, 2))
        .global_get(fd.0)
        .i32_const(IOVEC as i32)
        .i32_const(1)
        .i32_const(NWRITTEN as i32)
        .call(fd_write.0)
        .local_tee(result);
    f.if_(BlockType::Empty);
    f.ins().local_get(result).global_set(error.0);
    f.br(done);
    f.end();
    f.ins()
        .local_get(ptr)
        .i32_const(0)
        .i32_load(mem(NWRITTEN, 2))
        .i32_add()
        .local_set(ptr)
        .local_get(len)
        .i32_const(0)
        .i32_load(mem(NWRITTEN, 2))
        .i32_sub()
        .local_set(len);
    f.br(rest);
    f.end();
    f.end();
    f.ins().i32_const(BUFFER_START).global_set(pos.0);
    m.add_func(ty, f.finish())
}

fn add_to_stderr(m: &mut ModuleState, flush: FuncIdx, fd: GlobalIdx) -> FuncIdx {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    f.ins().call(flush.0).i32_const(STDERR).global_set(fd.0);
    m.add_func(ty, f.finish())
}

fn add_exit(m: &mut ModuleState, flush: FuncIdx, proc_exit: FuncIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let code = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .call(flush.0)
        .local_get(code)
        .call(proc_exit.0)
        .unreachable();
    m.add_func(ty, f.finish())
}

/// The function that encodes a code point in UTF-8 at the end of the buffer.
fn add_write_char(m: &mut ModuleState, flush: FuncIdx, pos: GlobalIdx) -> FuncIdx {
    let ty = m.func_type(&[ValType::I32], &[]);
    let c = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .global_get(pos.0)
        .i32_const(BUFFER_END - MAX_CHAR_BYTES)
        .i32_gt_u();
    f.if_(BlockType::Empty);
    f.ins().call(flush.0);
    f.end();
    // The bytes of each length, as (shift, mark) for the lead byte and the
    // continuation bytes after it.
    let encodings: [(i32, &[(i32, i32)]); 4] = [
        (0x80, &[(0, 0x00)]),
        (0x800, &[(6, 0xC0), (0, 0x80)]),
        (0x10000, &[(12, 0xE0), (6, 0x80), (0, 0x80)]),
        (0x110000, &[(18, 0xF0), (12, 0x80), (6, 0x80), (0, 0x80)]),
    ];
    for (limit, bytes) in encodings {
        f.ins().local_get(c).i32_const(limit).i32_lt_u();
        f.if_(BlockType::Empty);
        for (offset, (shift, mark)) in bytes.iter().enumerate() {
            let mut ins = f.ins();
            ins.global_get(pos.0).local_get(c);
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
            .global_get(pos.0)
            .i32_const(bytes.len() as i32)
            .i32_add()
            .global_set(pos.0)
            .return_();
        f.end();
    }
    f.ins().unreachable();
    m.add_func(ty, f.finish())
}
