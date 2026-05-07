//! Typed draw-list FFI exposed as the `_drawlist` Python module.
//!
//! Python builds a draw list incrementally by calling typed methods on a
//! `_drawlist.DrawList` object instead of formatting a text string. The
//! macros below let RustPython generate all the Python ↔ Rust argument
//! coercion automatically — no manual `try_into_value` plumbing.
//!
//! This skips both ends of the previous bottleneck:
//! - Python: no float-to-string formatting per command.
//! - Rust:   no whitespace-split + `parse::<f32>` per token.
//!
//! Native-only — on `wasm32` the host renders via SVG, so the typed pipeline
//! isn't compiled in.
#![cfg(not(target_arch = "wasm32"))]

pub(crate) use _drawlist::module_def;

#[pymodule(name = "_drawlist")]
mod _drawlist {
    use rustpython_vm::common::lock::PyMutex;
    use rustpython_vm::{
        Py, PyObjectRef, PyPayload, PyResult, VirtualMachine, builtins::PyType, types::Constructor,
    };
    use simage::ir::{
        ClipBox, DrawList, FillRule, FontItalic, LineCap, LineJoin, PathStyle, Rgba, TextNode,
    };

    #[pyattr]
    #[pyclass(name = "DrawList")]
    #[derive(PyPayload, Default)]
    pub struct PyDrawList {
        inner: PyMutex<DrawList>,
    }

    impl std::fmt::Debug for PyDrawList {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("DrawList").finish()
        }
    }

    /// `DrawList(width, height)`.
    #[derive(FromArgs)]
    pub struct DrawListNewArgs {
        #[pyarg(positional)]
        width: f32,
        #[pyarg(positional)]
        height: f32,
    }

    impl Constructor for PyDrawList {
        type Args = DrawListNewArgs;

        fn py_new(_cls: &Py<PyType>, args: Self::Args, _vm: &VirtualMachine) -> PyResult<Self> {
            Ok(PyDrawList {
                inner: PyMutex::new(DrawList::new(args.width, args.height)),
            })
        }
    }

    /// Args for [`PyDrawList::path_begin`]. The wire format keeps colors as
    /// `(r, g, b)` ints in `[0, 255]` plus alpha as `f32` in `[0, 1]`, so we
    /// avoid wrapping every call site in extra Python `Rgba` objects.
    #[derive(FromArgs)]
    pub struct PathBeginArgs {
        #[pyarg(positional)]
        fill_r: i32,
        #[pyarg(positional)]
        fill_g: i32,
        #[pyarg(positional)]
        fill_b: i32,
        #[pyarg(positional)]
        fill_a: f32,
        #[pyarg(positional)]
        stroke_r: i32,
        #[pyarg(positional)]
        stroke_g: i32,
        #[pyarg(positional)]
        stroke_b: i32,
        #[pyarg(positional)]
        stroke_a: f32,
        #[pyarg(positional)]
        stroke_width: f32,
        #[pyarg(positional)]
        line_cap: i32,
        #[pyarg(positional)]
        line_join: i32,
        #[pyarg(positional)]
        fill_rule: i32,
        #[pyarg(positional)]
        closed: i32,
    }

    /// Args for [`PyDrawList::text`].
    #[derive(FromArgs)]
    pub struct TextArgs {
        #[pyarg(positional)]
        fill_r: i32,
        #[pyarg(positional)]
        fill_g: i32,
        #[pyarg(positional)]
        fill_b: i32,
        #[pyarg(positional)]
        fill_a: f32,
        #[pyarg(positional)]
        stroke_r: i32,
        #[pyarg(positional)]
        stroke_g: i32,
        #[pyarg(positional)]
        stroke_b: i32,
        #[pyarg(positional)]
        stroke_a: f32,
        #[pyarg(positional)]
        stroke_width: f32,
        #[pyarg(positional)]
        line_cap: i32,
        #[pyarg(positional)]
        line_join: i32,
        #[pyarg(positional)]
        cx: f32,
        #[pyarg(positional)]
        cy: f32,
        #[pyarg(positional)]
        bw: f32,
        #[pyarg(positional)]
        bh: f32,
        #[pyarg(positional)]
        angle: f32,
        #[pyarg(positional)]
        flip_h: i32,
        #[pyarg(positional)]
        flip_v: i32,
        #[pyarg(positional)]
        size: f32,
        #[pyarg(positional)]
        italic: i32,
        #[pyarg(positional)]
        underline: i32,
        #[pyarg(positional)]
        text: String,
    }

    /// Args for [`PyDrawList::cubic_to`]. Six floats — over the auto-impl
    /// limit, so we wrap them.
    #[derive(FromArgs)]
    pub struct CubicArgs {
        #[pyarg(positional)]
        c1x: f32,
        #[pyarg(positional)]
        c1y: f32,
        #[pyarg(positional)]
        c2x: f32,
        #[pyarg(positional)]
        c2y: f32,
        #[pyarg(positional)]
        x: f32,
        #[pyarg(positional)]
        y: f32,
    }

    /// Args for [`PyDrawList::arc_to`]. Seven values; SVG endpoint arc form.
    #[derive(FromArgs)]
    pub struct ArcArgs {
        #[pyarg(positional)]
        rx: f32,
        #[pyarg(positional)]
        ry: f32,
        #[pyarg(positional)]
        rotation_deg: f32,
        #[pyarg(positional)]
        large_arc: i32,
        #[pyarg(positional)]
        sweep: i32,
        #[pyarg(positional)]
        x: f32,
        #[pyarg(positional)]
        y: f32,
    }

    fn rgba(r: i32, g: i32, b: i32, a: f32) -> Rgba {
        Rgba {
            r: r.clamp(0, 255) as u8,
            g: g.clamp(0, 255) as u8,
            b: b.clamp(0, 255) as u8,
            a,
        }
    }

    #[pyclass(with(Constructor))]
    impl PyDrawList {
        #[pymethod]
        fn path_begin(&self, a: PathBeginArgs) {
            self.inner.lock().path_begin(PathStyle {
                fill: rgba(a.fill_r, a.fill_g, a.fill_b, a.fill_a),
                stroke: rgba(a.stroke_r, a.stroke_g, a.stroke_b, a.stroke_a),
                stroke_width: a.stroke_width,
                line_cap: LineCap::from_u8(a.line_cap.clamp(0, 255) as u8),
                line_join: LineJoin::from_u8(a.line_join.clamp(0, 255) as u8),
                fill_rule: FillRule::from_u8(a.fill_rule.clamp(0, 255) as u8),
                closed: a.closed != 0,
            });
        }

        #[pymethod]
        fn move_to(&self, x: f32, y: f32) {
            self.inner.lock().move_to(x, y);
        }

        #[pymethod]
        fn line_to(&self, x: f32, y: f32) {
            self.inner.lock().line_to(x, y);
        }

        #[pymethod]
        fn quad_to(&self, cx: f32, cy: f32, x: f32, y: f32) {
            self.inner.lock().quad_to(cx, cy, x, y);
        }

        #[pymethod]
        fn cubic_to(&self, a: CubicArgs) {
            self.inner
                .lock()
                .cubic_to(a.c1x, a.c1y, a.c2x, a.c2y, a.x, a.y);
        }

        #[pymethod]
        fn arc_to(&self, a: ArcArgs) {
            self.inner.lock().arc_to(
                a.rx,
                a.ry,
                a.rotation_deg,
                a.large_arc != 0,
                a.sweep != 0,
                a.x,
                a.y,
            );
        }

        #[pymethod]
        fn path_end(&self) {
            self.inner.lock().path_end();
        }

        #[pymethod]
        fn clip_push(&self, cx: f32, cy: f32, w: f32, h: f32, angle: f32) {
            self.inner.lock().clip_push(ClipBox {
                cx,
                cy,
                w,
                h,
                angle,
            });
        }

        #[pymethod]
        fn clip_pop(&self) {
            self.inner.lock().clip_pop();
        }

        #[pymethod]
        fn text(&self, a: TextArgs) {
            self.inner.lock().text(TextNode {
                fill: rgba(a.fill_r, a.fill_g, a.fill_b, a.fill_a),
                stroke: rgba(a.stroke_r, a.stroke_g, a.stroke_b, a.stroke_a),
                stroke_width: a.stroke_width,
                line_cap: LineCap::from_u8(a.line_cap.clamp(0, 255) as u8),
                line_join: LineJoin::from_u8(a.line_join.clamp(0, 255) as u8),
                cx: a.cx,
                cy: a.cy,
                bw: a.bw,
                bh: a.bh,
                angle: a.angle,
                flip_h: a.flip_h != 0,
                flip_v: a.flip_v != 0,
                size: a.size,
                italic: FontItalic::from_u8(a.italic.clamp(0, 255) as u8),
                underline: a.underline != 0,
                text: a.text,
            });
        }

        #[pymethod]
        fn bitmap(&self) {
            self.inner.lock().bitmap();
        }

        /// Render to the active terminal backend (Kitty / Sixel / half-blocks).
        #[pymethod]
        fn show(&self) {
            simage::world_term::show_image_dl(&self.inner.lock());
        }

        /// Render to PDF bytes.
        #[pymethod]
        fn to_pdf(&self, vm: &VirtualMachine) -> PyObjectRef {
            let bytes = simage::pdf::render_to_pdf_dl(&self.inner.lock());
            vm.ctx.new_bytes(bytes).into()
        }
    }
}
