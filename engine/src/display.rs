//! The `_spython_display` module: the display that a World draws into, over
//! [`sinteract::display::Frames`], which is a window, the terminal, or a
//! session with a server, and natively a printer of images in the terminal. Both draw
//! the [`Scene`] that [`crate::scene::draw`] builds, so Python never holds a
//! scene that is half built.

pub(crate) use _spython_display::module_def;

/// Ends the display of a world that a run left open. On the page a Stop
/// stays on until the next run, so it also stops the `finally` of the world
/// that would close the display.
pub(crate) fn close_frames() {
    // The terminal of a world keeps no rows under the frame.
    let _ = _spython_display::FRAMES.with_borrow_mut(|frames| frames.close());
}

#[rustpython_vm::pymodule(name = "_spython_display")]
mod _spython_display {
    use std::cell::RefCell;
    use std::thread;
    use std::time::{Duration, Instant};

    use rustpython_vm::builtins::PyType;
    #[cfg(not(target_arch = "wasm32"))]
    use rustpython_vm::common::lock::PyMutex;
    use rustpython_vm::function::ArgIntoFloat;
    use rustpython_vm::types::Constructor;
    use rustpython_vm::{Py, PyObjectRef, PyPayload, PyResult, VirtualMachine, pyclass};
    use sinteract::display::{FrameEvent, Frames, StageOptions};
    #[cfg(not(target_arch = "wasm32"))]
    use sinteract::display::{PrintError, Printer, TerminalOptions};
    use sinteract::scene::Scene;

    use crate::scene::draw;

    /// The kind of the close in what `wait_event` returns, first in its
    /// tuple. A key keeps the number of its
    /// [`KeyKind`](sinteract::event::KeyKind), which `spython.world` reads.
    const CLOSE: i32 = 3;

    /// How long `wait_event` blocks before it runs the handlers of the
    /// signals that arrived. Python runs them between two bytecodes, and a
    /// window gets its Ctrl-C from the terminal that started the program.
    const SIGNAL_PERIOD: Duration = Duration::from_millis(100);

    /// A printer of images at the cursor, or `None` when the terminal shows
    /// no graphics or stdout is not a terminal. The page draws an svg
    /// instead, so it has no printer.
    #[cfg(not(target_arch = "wasm32"))]
    #[pyfunction]
    fn open_printer(vm: &VirtualMachine) -> Option<PyObjectRef> {
        let printer = Printer::new().ok()?;
        let printer = PyPrinter {
            inner: PyMutex::new(printer),
        };
        Some(printer.into_ref(&vm.ctx).into())
    }

    /// Prints one image after another in the terminal.
    #[cfg(not(target_arch = "wasm32"))]
    #[pyattr]
    #[pyclass(name = "Printer")]
    #[derive(PyPayload)]
    pub struct PyPrinter {
        inner: PyMutex<Printer>,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl std::fmt::Debug for PyPrinter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Printer")
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[pyclass]
    impl PyPrinter {
        /// Print `image` at the cursor, on a scene of `width` by `height`.
        /// Returns `True` if it printed, and `False` while a display holds
        /// the terminal.
        #[pymethod]
        fn show(
            &self,
            image: PyObjectRef,
            width: ArgIntoFloat,
            height: ArgIntoFloat,
            vm: &VirtualMachine,
        ) -> PyResult<bool> {
            let mut scene = Scene::new(f64::from(width) as f32, f64::from(height) as f32);
            draw(&image, &mut scene, vm)?;
            match self.inner.lock().print(&scene) {
                Ok(()) => Ok(true),
                Err(PrintError::Busy) => Ok(false),
                Err(e @ (PrintError::Alloc(_) | PrintError::Io(_))) => {
                    Err(vm.new_runtime_error(e.to_string()))
                }
            }
        }
    }

    thread_local! {
        /// The display of the worlds of a run, on the thread that runs
        /// Python, since a window of winit stays on the thread that made it.
        pub(super) static FRAMES: RefCell<Frames> =
            RefCell::new(Frames::new("spython", stage_options));
    }

    /// The page shows a world in a session, the only display it has.
    fn stage_options() -> StageOptions {
        StageOptions {
            #[cfg(not(target_arch = "wasm32"))]
            terminal: TerminalOptions {
                keep_last_frame: true,
                ..TerminalOptions::default()
            },
            ..StageOptions::default()
        }
    }

    /// The display of a world. The first `show` opens a window, and the
    /// terminal when no window opens, or the session when a server runs
    /// the program. The terminal keeps the last frame on screen after the
    /// close.
    #[pyattr]
    #[pyclass(name = "Display")]
    #[derive(Debug, PyPayload)]
    pub struct PyDisplay;

    impl Constructor for PyDisplay {
        type Args = ();

        fn py_new(_cls: &Py<PyType>, _args: Self::Args, _vm: &VirtualMachine) -> PyResult<Self> {
            Ok(PyDisplay)
        }
    }

    #[pyclass(with(Constructor))]
    impl PyDisplay {
        /// Wait at most `ms` milliseconds for a key or the close, and
        /// return it as `(kind, key, alt, ctrl, shift, meta)`, or `None`
        /// when the time runs out. Only a key fills the fields after the
        /// kind. A signal that arrives meanwhile runs its
        /// handler, so Ctrl-C raises `KeyboardInterrupt`. A message that
        /// the session drops goes to stderr, and the wait goes on.
        #[pymethod]
        fn wait_event(&self, ms: u64, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
            let end = Instant::now() + Duration::from_millis(ms);
            let event = loop {
                let deadline = (Instant::now() + SIGNAL_PERIOD).min(end);
                let event = FRAMES.with_borrow_mut(|frames| {
                    if frames.is_open() {
                        frames.wait_event(deadline, |e| eprintln!("spython: {e}"))
                    } else {
                        // No world shows a frame, so there is nothing to
                        // wait for but the time.
                        thread::sleep(deadline.saturating_duration_since(Instant::now()));
                        FrameEvent::Timeout
                    }
                });
                if event != FrameEvent::Timeout {
                    break event;
                }
                if Instant::now() >= end {
                    return Ok(vm.ctx.none());
                }
                vm.check_signals()?;
            };
            let (kind, key) = match event {
                FrameEvent::Key(k) => (k.kind as i32, Some(k)),
                FrameEvent::Close => (CLOSE, None),
                FrameEvent::Timeout => unreachable!("the loop above takes each Timeout"),
            };
            let (name, mods) = match key {
                Some(k) => (k.key, k.modifiers),
                None => (String::new(), Default::default()),
            };
            let elements = vec![
                vm.ctx.new_int(kind).into(),
                vm.ctx.new_str(name).into(),
                vm.ctx.new_bool(mods.alt).into(),
                vm.ctx.new_bool(mods.ctrl).into(),
                vm.ctx.new_bool(mods.shift).into(),
                vm.ctx.new_bool(mods.meta).into(),
            ];
            Ok(vm.ctx.new_tuple(elements).into())
        }

        /// Show `image`, an `Image` of `spython.image`, on a scene of
        /// `width` by `height`, at the next frame of the display. The first
        /// call opens the display.
        #[pymethod]
        fn show(
            &self,
            image: PyObjectRef,
            width: ArgIntoFloat,
            height: ArgIntoFloat,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            let mut scene = Scene::new(f64::from(width) as f32, f64::from(height) as f32);
            draw(&image, &mut scene, vm)?;
            FRAMES
                .with_borrow_mut(|frames| frames.show(scene))
                .map_err(|e| vm.new_runtime_error(e.to_string()))
        }

        /// Show the last image, and end the display. A second call does
        /// nothing.
        #[pymethod]
        fn close(&self) {
            super::close_frames();
        }
    }
}
