//! What the module asks of the WASI host: the standard output, written
//! through a buffer in linear memory.

use wasm_encoder::{BlockType, ConstExpr, MemArg, MemArgTarget, ValType};

use super::module::{FuncState, ModuleState};

/// Where `fd_write` reads the one iovec of a write.
const IOVEC: u64 = 0;
/// Where `fd_write` stores the number of bytes it wrote.
const NWRITTEN: u64 = 8;
const BUFFER_START: i32 = 16;
const BUFFER_END: i32 = 65536;
/// The longest UTF-8 encoding of a code point.
const MAX_CHAR_BYTES: i32 = 4;
const STDOUT: i32 = 1;

pub(crate) struct Host {
    /// `(func (param i32))`, which writes a code point to the standard output.
    pub write_char: u32,
    /// `(func)`, which sends what the buffer holds to the host.
    pub flush: u32,
}

impl Host {
    pub fn new(m: &mut ModuleState) -> Self {
        let fd_write_ty = m.func_type(
            &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I32],
        );
        let fd_write = m.import_func("wasi_snapshot_preview1", "fd_write", fd_write_ty);
        // WASI reads the buffers from the memory the module exports by this name.
        m.memory("memory");
        let pos = m.global(ValType::I32, &ConstExpr::i32_const(BUFFER_START));
        let flush = add_flush(m, fd_write, pos);
        let write_char = add_write_char(m, flush, pos);
        Host { write_char, flush }
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

fn add_flush(m: &mut ModuleState, fd_write: u32, pos: u32) -> u32 {
    let ty = m.func_type(&[], &[]);
    let mut f = FuncState::new(0);
    let ptr = f.local(ValType::I32);
    let len = f.local(ValType::I32);
    f.ins()
        .i32_const(BUFFER_START)
        .local_set(ptr)
        .global_get(pos)
        .i32_const(BUFFER_START)
        .i32_sub()
        .local_set(len)
        // A write can take fewer bytes than it was given, so the loop asks
        // again for the rest. An error drops the rest, because the program
        // has no exceptions to raise yet.
        .block(BlockType::Empty)
        .loop_(BlockType::Empty)
        .local_get(len)
        .i32_eqz()
        .br_if(1)
        .i32_const(0)
        .local_get(ptr)
        .i32_store(mem(IOVEC, 2))
        .i32_const(0)
        .local_get(len)
        .i32_store(mem(IOVEC + 4, 2))
        .i32_const(STDOUT)
        .i32_const(IOVEC as i32)
        .i32_const(1)
        .i32_const(NWRITTEN as i32)
        .call(fd_write)
        .br_if(1)
        .local_get(ptr)
        .i32_const(0)
        .i32_load(mem(NWRITTEN, 2))
        .i32_add()
        .local_set(ptr)
        .local_get(len)
        .i32_const(0)
        .i32_load(mem(NWRITTEN, 2))
        .i32_sub()
        .local_set(len)
        .br(0)
        .end()
        .end()
        .i32_const(BUFFER_START)
        .global_set(pos);
    m.add_func(ty, f.finish())
}

/// The function that encodes a code point in UTF-8 at the end of the buffer.
fn add_write_char(m: &mut ModuleState, flush: u32, pos: u32) -> u32 {
    let ty = m.func_type(&[ValType::I32], &[]);
    let c = 0;
    let mut f = FuncState::new(1);
    f.ins()
        .global_get(pos)
        .i32_const(BUFFER_END - MAX_CHAR_BYTES)
        .i32_gt_u()
        .if_(BlockType::Empty)
        .call(flush)
        .end();
    // The bytes of each length, as (shift, mark) for the lead byte and the
    // continuation bytes after it.
    let encodings: [(i32, &[(i32, i32)]); 4] = [
        (0x80, &[(0, 0x00)]),
        (0x800, &[(6, 0xC0), (0, 0x80)]),
        (0x10000, &[(12, 0xE0), (6, 0x80), (0, 0x80)]),
        (0x110000, &[(18, 0xF0), (12, 0x80), (6, 0x80), (0, 0x80)]),
    ];
    for (limit, bytes) in encodings {
        f.ins()
            .local_get(c)
            .i32_const(limit)
            .i32_lt_u()
            .if_(BlockType::Empty);
        for (offset, (shift, mark)) in bytes.iter().enumerate() {
            let mut ins = f.ins();
            ins.global_get(pos).local_get(c);
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
            .global_get(pos)
            .i32_const(bytes.len() as i32)
            .i32_add()
            .global_set(pos)
            .return_()
            .end();
    }
    f.ins().unreachable();
    m.add_func(ty, f.finish())
}
