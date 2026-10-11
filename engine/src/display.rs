//! The `_spython_display` module: the display that a World draws into, over
//! [`sinteract::display::Frames`], which is a window, the terminal, or a
//! session with a server, and natively a printer of images in the terminal. Both draw
//! the [`Scene`] that [`crate::scene::draw`] builds, so Python never holds a
//! scene that is half built.

pub(crate) use _spython_display::module_def;

#[rustpython_vm::pymodule(name = "_spython_display")]
mod _spython_display {
    use std::thread::{self, ThreadId};
    use std::time::{Duration, Instant};

    use rustpython_vm::builtins::PyType;
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

    #[pyattr]
    #[pyclass(name = "Display")]
    #[derive(Debug, PyPayload)]
    pub struct PyDisplay {
        inner: PyMutex<OnThread<Frames>>,
    }

    impl Constructor for PyDisplay {
        type Args = (String,);

        /// The display of a world, with `title` on its window. The first
        /// `show` opens a window, and the terminal when no window opens,
        /// or the session when a server runs the program. The terminal
        /// keeps the last frame on screen after the close.
        fn py_new(_cls: &Py<PyType>, (title,): Self::Args, _vm: &VirtualMachine) -> PyResult<Self> {
            // The page shows a world in a session, the only display it has.
            let frames = Frames::new(title, || StageOptions {
                #[cfg(not(target_arch = "wasm32"))]
                terminal: TerminalOptions {
                    keep_last_frame: true,
                    ..TerminalOptions::default()
                },
                ..StageOptions::default()
            });
            Ok(PyDisplay {
                inner: PyMutex::new(OnThread::new(frames)),
            })
        }
    }

    #[pyclass(with(Constructor))]
    impl PyDisplay {
        /// Wait at most `ms` milliseconds for a key or the close, and
        /// return it as `(kind, key, alt, ctrl, shift, meta, repeat)`, or
        /// `None` when the time runs out. Only a key fills the fields after
        /// the kind, and `repeat` is always `False`, because a repeat comes
        /// as another press. A signal that arrives meanwhile runs its
        /// handler, so Ctrl-C raises `KeyboardInterrupt`. A message that
        /// the session drops goes to stderr, and the wait goes on.
        #[pymethod]
        fn wait_event(&self, ms: u64, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
            let end = Instant::now() + Duration::from_millis(ms);
            let event = loop {
                let deadline = (Instant::now() + SIGNAL_PERIOD).min(end);
                let mut inner = self.inner.lock();
                let frames = inner.get_mut();
                if !frames.is_open() {
                    // No world shows a frame, so there is nothing to wait
                    // for but the time.
                    thread::sleep(deadline.saturating_duration_since(Instant::now()));
                } else {
                    match frames.wait_event(deadline, |e| eprintln!("spython: {e}")) {
                        FrameEvent::Timeout => {}
                        event => break event,
                    }
                }
                drop(inner);
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
                vm.ctx.new_bool(false).into(),
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
            self.inner
                .lock()
                .get_mut()
                .show(scene)
                .map_err(|e| vm.new_runtime_error(e.to_string()))
        }

        /// Show the last image, and end the display. A second call does
        /// nothing.
        #[pymethod]
        fn close(&self) {
            // The terminal of a world keeps no rows under the frame.
            let _ = self.inner.lock().get_mut().close();
        }
    }

    /// A value that stays on the thread that made it. A window of winit is
    /// not `Send`, and a pyclass has to be, so each access checks the
    /// thread.
    struct OnThread<T> {
        owner: ThreadId,
        value: T,
    }

    // SAFETY: `get_mut` and `drop` are the only ways to the value, and both
    // assert that they run on `owner`.
    unsafe impl<T> Send for OnThread<T> {}
    unsafe impl<T> Sync for OnThread<T> {}

    impl<T> OnThread<T> {
        fn new(value: T) -> Self {
            Self {
                owner: thread::current().id(),
                value,
            }
        }

        fn get_mut(&mut self) -> &mut T {
            self.assert_owner();
            &mut self.value
        }

        fn assert_owner(&self) {
            assert_eq!(
                thread::current().id(),
                self.owner,
                "a Display is used on the thread that opened it"
            );
        }
    }

    impl<T> Drop for OnThread<T> {
        fn drop(&mut self) {
            self.assert_owner();
        }
    }

    impl<T> std::fmt::Debug for OnThread<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("OnThread").finish_non_exhaustive()
        }
    }
}
