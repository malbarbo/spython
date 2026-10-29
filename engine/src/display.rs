//! The `_spython_display` module: the display that a World draws into, over
//! [`sinteract::display::Worlds`], which is a window, the terminal, or a
//! session with a server, and natively a printer of images in the terminal.
//! Both draw the [`sinteract::scene::Scene`] of an `Image`, so Python never
//! holds a scene that is half built.

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
    use std::time::{Duration, Instant};

    use rustpython_vm::builtins::PyModule;
    #[cfg(not(target_arch = "wasm32"))]
    use rustpython_vm::builtins::PyType;
    #[cfg(not(target_arch = "wasm32"))]
    use rustpython_vm::common::lock::PyMutex;
    #[cfg(not(target_arch = "wasm32"))]
    use rustpython_vm::types::Constructor;
    use rustpython_vm::{Py, PyRef, PyResult, VirtualMachine};
    #[cfg(not(target_arch = "wasm32"))]
    use rustpython_vm::{PyPayload, pyclass};
    #[cfg(not(target_arch = "wasm32"))]
    use sinteract::display::{NoGraphics, PrintError, Printer, TerminalOptions};
    use sinteract::display::{StageMode, WorldEvent, Worlds};
    use sinteract::event::KeyKind;

    use crate::image::PyImage;

    /// The kind of the close in what `wait_event` returns, first in its
    /// tuple. A key keeps the number of its [`KeyKind`].
    const CLOSE: i32 = 3;

    /// The module, with the kinds of event as `KEYPRESS`, `KEYDOWN`, `KEYUP`
    /// and `CLOSE`. The code of a `#[pyattr]` const calls `unwrap`, which
    /// clippy denies here.
    pub(crate) fn module_exec(vm: &VirtualMachine, module: &Py<PyModule>) -> PyResult<()> {
        __module_exec(vm, module);
        let kinds = [
            ("KEYPRESS", KeyKind::Press as i32),
            ("KEYDOWN", KeyKind::Down as i32),
            ("KEYUP", KeyKind::Up as i32),
            ("CLOSE", CLOSE),
        ];
        for (name, kind) in kinds {
            module.set_attr(name, vm.new_pyobj(kind), vm)?;
        }
        Ok(())
    }

    /// How long `wait_event` blocks before it runs the handlers of the
    /// signals that arrived. Python runs them between two bytecodes, and a
    /// window gets its Ctrl-C from the terminal that started the program.
    const SIGNAL_PERIOD: Duration = Duration::from_millis(100);

    /// Prints one image after another in the terminal. The page draws an
    /// svg instead, so it has no printer.
    #[cfg(not(target_arch = "wasm32"))]
    #[pyattr]
    #[pyclass(name = "Printer")]
    #[derive(PyPayload)]
    pub struct PyPrinter {
        /// `None` until the first show, since the probe of the terminal
        /// waits for its answer.
        inner: PyMutex<Option<Result<Printer, NoGraphics>>>,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl std::fmt::Debug for PyPrinter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Printer")
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Constructor for PyPrinter {
        type Args = ();

        fn py_new(_cls: &Py<PyType>, _args: (), _vm: &VirtualMachine) -> PyResult<Self> {
            Ok(PyPrinter {
                inner: PyMutex::new(None),
            })
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[pyclass(with(Constructor))]
    impl PyPrinter {
        /// Print `image` at the cursor, on the canvas that a repl shows it
        /// on. Returns `True` if it printed, and `False` when the terminal
        /// shows no graphics, while a display holds the terminal, or when
        /// the image is too large to draw.
        #[pymethod]
        fn show(&self, image: PyRef<PyImage>, vm: &VirtualMachine) -> PyResult<bool> {
            let mut printer = self.inner.lock();
            let Ok(printer) = printer.get_or_insert_with(Printer::new) else {
                return Ok(false);
            };
            let Ok(scene) = image.0.to_repl_scene() else {
                return Ok(false);
            };
            match printer.print(&scene) {
                Ok(()) => Ok(true),
                Err(PrintError::Busy | PrintError::Alloc(_)) => Ok(false),
                Err(e @ PrintError::Io(_)) => Err(vm.new_runtime_error(e.to_string())),
            }
        }
    }

    thread_local! {
        /// The display of the worlds of a run, on the thread that runs
        /// Python, since a window of winit stays on the thread that made it.
        pub(super) static FRAMES: RefCell<Worlds> =
            RefCell::new(Worlds::new("spython", stage_mode));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn stage_mode() -> StageMode {
        let terminal = TerminalOptions {
            keep_last_frame: true,
            ..TerminalOptions::default()
        };
        StageMode::auto(None, terminal)
    }

    /// The page shows a world in a session, the only display it has.
    #[cfg(target_arch = "wasm32")]
    fn stage_mode() -> StageMode {
        StageMode::Session
    }

    /// Wait at most `ms` milliseconds for a key or the close, and
    /// return it as `(kind, key)`, or `None` when the time runs out. The
    /// close has the empty key. A signal that arrives meanwhile runs its
    /// handler, so Ctrl-C raises `KeyboardInterrupt`. A message that the
    /// session drops goes to stderr, and the wait goes on.
    #[pyfunction]
    fn wait_event(ms: u64, vm: &VirtualMachine) -> PyResult<Option<(i32, String)>> {
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            let deadline = (Instant::now() + SIGNAL_PERIOD).min(end);
            let event = FRAMES
                .with_borrow_mut(|frames| frames.wait(deadline, |e| eprintln!("spython: {e}")));
            match event {
                WorldEvent::Key(_, k) => return Ok(Some((k.kind as i32, k.key))),
                WorldEvent::Close => return Ok(Some((CLOSE, String::new()))),
                WorldEvent::Pad(..) | WorldEvent::Frame | WorldEvent::Timeout => {}
            }
            if Instant::now() >= end {
                return Ok(None);
            }
            vm.check_signals()?;
        }
    }

    /// Show `image` on its canvas at the next frame of the display. The
    /// first call opens a window, the terminal when no window opens, or the
    /// session when a server runs the program.
    #[pyfunction]
    fn show(image: PyRef<PyImage>, vm: &VirtualMachine) -> PyResult<()> {
        let scene = image
            .0
            .to_scene()
            .map_err(|e| vm.new_value_error(e.to_string()))?;
        FRAMES
            .with_borrow_mut(|frames| frames.show(scene))
            .map_err(|e| vm.new_runtime_error(e.to_string()))
    }

    /// Show the last image, and end the display. The terminal keeps the
    /// last image on screen. A second call does nothing.
    #[pyfunction]
    fn close() {
        super::close_frames();
    }
}
