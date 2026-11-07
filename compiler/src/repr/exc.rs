//! The exceptions, and the traceback of an exception that nothing catches.

use wasm_encoder::{BlockType, Catch, ConstExpr, HeapType, RefType, StorageType, ValType};

use super::str::StrRepr;
use crate::emit::host::Host;
use crate::emit::module::{FuncState, ModuleState};

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
    /// `(array (mut (ref null $Str)))`, with the text of the frame of each
    /// site.
    sites: u32,
    /// The global that holds the array of the sites.
    sites_global: u32,
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

        let raise_type = m.func_type(&[ref_to(str_type, false)], &[]);
        let mut f = FuncState::new(1);
        f.ins()
            .local_get(0)
            .ref_null(HeapType::Concrete(frame))
            .struct_new(exc)
            .throw(tag);
        let raise = m.add_func(raise_type, f.finish());

        let uncaught_type = m.func_type(&[ref_to(exc, false), ValType::I32], &[]);
        let (e, site) = (0, 1);
        let mut f = FuncState::new(2);
        let next = f.local(ref_to(frame, true));
        f.ins()
            .local_get(e)
            .local_get(site)
            .local_get(e)
            .struct_get(exc, 1)
            .struct_new(frame)
            .struct_set(exc, 1)
            .call(host.to_stderr);
        str.literal(m, &mut f, "Traceback (most recent call last):\n");
        str.write(&mut f);
        f.ins()
            .local_get(e)
            .struct_get(exc, 1)
            .local_set(next)
            .block(BlockType::Empty)
            .loop_(BlockType::Empty)
            .local_get(next)
            .ref_is_null()
            .br_if(1)
            .global_get(sites_global)
            .local_get(next)
            .struct_get(frame, 0)
            .array_get(sites)
            .ref_as_non_null();
        str.write(&mut f);
        f.ins()
            .local_get(next)
            .struct_get(frame, 1)
            .local_set(next)
            .br(0)
            .end()
            .end()
            .local_get(e)
            .struct_get(exc, 0);
        str.write(&mut f);
        f.ins()
            .i32_const('\n' as i32)
            .call(host.write_char)
            .i32_const(1)
            .call(host.exit);
        let uncaught = m.add_func(uncaught_type, f.finish());

        Exceptions {
            exc,
            tag,
            raise,
            uncaught,
            sites,
            sites_global,
        }
    }

    /// Pops the message and raises an exception with it.
    pub fn raise(&self, f: &mut FuncState) {
        f.ins().call(self.raise).unreachable();
    }

    /// Opens the code of the module, where an exception that nothing
    /// catches ends the program.
    pub fn begin_module(&self, f: &mut FuncState) {
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

    /// Closes what [`Self::begin_module`] opened. `site` is the local that
    /// holds the site of the instruction that may raise.
    pub fn end_module(&self, f: &mut FuncState, site: u32) {
        f.ins()
            .end()
            .return_()
            .end()
            .local_get(site)
            .call(self.uncaught);
    }

    /// Fills the array of the sites, with the text of the frame of each.
    pub fn finish(&self, m: &mut ModuleState, str: &mut StrRepr, sites: &[String]) {
        let globals: Vec<u32> = sites
            .iter()
            .map(|text| str.literal_global(m, text))
            .collect();
        let len = i32::try_from(sites.len()).expect("fewer than 2^31 sites");
        let mut ins = wasm_encoder::InstructionSink::new(&mut m.init);
        ins.i32_const(len)
            .array_new_default(self.sites)
            .global_set(self.sites_global);
        for (i, global) in globals.into_iter().enumerate() {
            ins.global_get(self.sites_global)
                .i32_const(i32::try_from(i).expect("fewer than 2^31 sites"))
                .global_get(global)
                .array_set(self.sites);
        }
    }
}

fn ref_to(index: u32, nullable: bool) -> ValType {
    ValType::Ref(RefType {
        nullable,
        heap_type: HeapType::Concrete(index),
    })
}
