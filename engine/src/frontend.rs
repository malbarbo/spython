//! `_frontend.Frontend` pyclass — Python-owned handle to a
//! [`simage::frontend::Frontend`]. Replaces the previous process-wide
//! `OnceLock<Mutex<Frontend>>` in `host::native`.
//!
//! Construction is cheap in both modes; `WindowFrontend` does not actually
//! open a window until `enter()` is called. Python code (`spython.system`,
//! `spython.world`) builds a Frontend per use site — World owns one for
//! the duration of `run()`; the displayhook builds a transient one per
//! `show()` call.
//!
//! Native-only — on `wasm32` the typed pipeline is not compiled in (see
//! `drawlist.rs`).
#![cfg(not(target_arch = "wasm32"))]

pub(crate) use _frontend::module_def;

#[pymodule(name = "_frontend")]
mod _frontend {
    use std::time::{Duration, Instant};

    use rustpython_vm::common::lock::PyMutex;
    use rustpython_vm::{
        Py, PyObjectRef, PyPayload, PyRef, PyResult, VirtualMachine, builtins::PyType,
        function::OptionalArg, types::Constructor,
    };
    use simage::event::InputEvent;
    use simage::frontend::Frontend;

    use crate::drawlist::PyDrawList;

    #[pyattr]
    #[pyclass(name = "Frontend")]
    #[derive(PyPayload)]
    pub struct PyFrontend {
        inner: PyMutex<Frontend>,
    }

    impl std::fmt::Debug for PyFrontend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Frontend").finish()
        }
    }

    impl Default for PyFrontend {
        fn default() -> Self {
            PyFrontend {
                inner: PyMutex::new(crate::host::make_frontend()),
            }
        }
    }

    impl Constructor for PyFrontend {
        type Args = ();

        fn py_new(_cls: &Py<PyType>, _args: Self::Args, _vm: &VirtualMachine) -> PyResult<Self> {
            Ok(PyFrontend::default())
        }
    }

    #[pyclass(with(Constructor))]
    impl PyFrontend {
        /// Notify the underlying backend that an animation loop is starting
        /// (terminal: switch to the alt screen; window: open the window).
        #[pymethod]
        fn enter(&self) {
            self.inner.lock().enter();
        }

        /// Tear down what `enter` set up.
        #[pymethod]
        fn exit(&self) {
            self.inner.lock().exit();
        }

        /// Block until the next input event. `deadline_ms` caps the wait
        /// (relative to now); omitted means "wait forever".
        ///
        /// Returns `(kind, key, alt, ctrl, shift, meta, repeat)` or `None`
        /// when the frontend has shut down (window closed, stdin EOF).
        ///
        /// Event kinds: `0` = KeyPress, `1` = KeyDown, `2` = KeyUp,
        /// `3` = Vsync, `4` = Close. For Vsync and Close, `key` is empty
        /// and modifiers are all `false`.
        #[pymethod]
        fn wait_event(
            &self,
            deadline_ms: OptionalArg<i64>,
            vm: &VirtualMachine,
        ) -> PyObjectRef {
            let deadline = match deadline_ms {
                OptionalArg::Present(ms) => {
                    Some(Instant::now() + Duration::from_millis(ms.max(0) as u64))
                }
                OptionalArg::Missing => None,
            };
            let ev = self.inner.lock().wait_event(deadline);
            match ev {
                None => vm.ctx.none(),
                Some(ev) => {
                    let (kind, key, mods) = input_event_to_tuple(ev);
                    let elements = vec![
                        vm.ctx.new_int(kind).into(),
                        vm.ctx.new_str(key).into(),
                        vm.ctx.new_bool(mods[0]).into(), // alt
                        vm.ctx.new_bool(mods[1]).into(), // ctrl
                        vm.ctx.new_bool(mods[2]).into(), // shift
                        vm.ctx.new_bool(mods[3]).into(), // meta
                        vm.ctx.new_bool(mods[4]).into(), // repeat
                    ];
                    vm.ctx.new_tuple(elements).into()
                }
            }
        }

        /// Render a [`PyDrawList`] to this Frontend.
        #[pymethod]
        fn present(&self, dl: PyRef<PyDrawList>) {
            self.inner.lock().present(&dl.inner.lock());
        }

        /// Show an SVG image. In terminal mode this goes through the
        /// per-backend writer (kitty rasterizer or plain stdout). Window
        /// mode prints SVG text — the window itself only renders typed
        /// DrawLists, and the SVG path is the REPL/displayhook fallback
        /// for hosts without raster graphics.
        #[pymethod]
        fn show_svg(&self, svg: String) {
            match *self.inner.lock() {
                Frontend::Terminal(_) => simage::terminal::show_svg(&svg),
                Frontend::Window(_) | Frontend::Stdio(_) => println!("{svg}"),
            }
        }

        /// Whether the host terminal speaks the Kitty graphics protocol.
        /// Always `false` in window mode (the window is the surface).
        #[pymethod]
        fn kitty_supported(&self) -> bool {
            matches!(*self.inner.lock(), Frontend::Terminal(_))
                && simage::terminal::kitty_supported()
        }

        /// Whether the host terminal supports DEC Sixel. Always `false` in
        /// window mode.
        #[pymethod]
        fn sixel_supported(&self) -> bool {
            matches!(*self.inner.lock(), Frontend::Terminal(_))
                && simage::sixel::sixel_supported()
        }

        /// Whether the host supports raster graphics output. Always `true`
        /// in window mode (the window is itself raster); in terminal mode
        /// it reflects truecolor half-blocks support.
        #[pymethod]
        fn text_blocks_supported(&self) -> bool {
            match *self.inner.lock() {
                Frontend::Window(_) => true,
                Frontend::Terminal(_) => simage::terminal::text_blocks_supported(),
                Frontend::Stdio(_) => false,
            }
        }
    }

    fn input_event_to_tuple(ev: InputEvent) -> (i32, String, [bool; 5]) {
        match ev {
            InputEvent::Key(k) => {
                let kind = k.kind as i32;
                let mods = [k.alt(), k.ctrl(), k.shift(), k.meta(), k.repeat()];
                (kind, k.key, mods)
            }
            InputEvent::Vsync => (3, String::new(), [false; 5]),
            InputEvent::Close => (4, String::new(), [false; 5]),
        }
    }
}
