//! The `_spython_display` module: the stage that a World draws into, over
//! [`sinteract::display::Stage`], which is a window, the terminal, or a
//! session with a server, and a printer of images in the terminal. Both draw
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
    use sinteract::display::{PrintError, Printer, Stage, StageEvent, TerminalOptions};
    use sinteract::event::{InputEvent, Interrupt};
    use sinteract::scene::Scene;
    use sinteract::session::{PlayerRange, Target};

    use crate::scene::draw;

    /// The kinds of event that `wait_event` returns, first in its tuple. A
    /// key keeps the number of its [`KeyKind`](sinteract::event::KeyKind),
    /// which `spython.world` reads.
    const VSYNC: i32 = 3;
    const CLOSE: i32 = 4;
    const RESIZE: i32 = 5;
    const MOUSE: i32 = 6;
    const PAD: i32 = 7;

    /// How long `wait_event` blocks before it runs the handlers of the
    /// signals that arrived. Python runs them between two bytecodes, and a
    /// window gets its Ctrl-C from the terminal that started the program.
    const SIGNAL_PERIOD: Duration = Duration::from_millis(100);

    /// A World has one player, the user or player 1 of the server.
    const PLAYERS: PlayerRange = PlayerRange::new(1, 1).expect("1 to 1 is a range");

    /// A printer of images at the cursor, or `None` when the terminal shows
    /// no graphics or stdout is not a terminal.
    #[pyfunction]
    fn open_printer(vm: &VirtualMachine) -> Option<PyObjectRef> {
        let printer = Printer::new().ok()?;
        let printer = PyPrinter {
            inner: PyMutex::new(printer),
        };
        Some(printer.into_ref(&vm.ctx).into())
    }

    /// Prints one image after another in the terminal.
    #[pyattr]
    #[pyclass(name = "Printer")]
    #[derive(PyPayload)]
    pub struct PyPrinter {
        inner: PyMutex<Printer>,
    }

    impl std::fmt::Debug for PyPrinter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Printer")
        }
    }

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
        inner: PyMutex<OnThread<Stage>>,
    }

    impl Constructor for PyDisplay {
        type Args = (String, ArgIntoFloat, ArgIntoFloat);

        /// Open a window of `width` by `height`, and the terminal when no
        /// window opens, or the session when a server runs the program.
        /// The terminal keeps the last frame on screen after the close.
        fn py_new(
            _cls: &Py<PyType>,
            (title, width, height): Self::Args,
            vm: &VirtualMachine,
        ) -> PyResult<Self> {
            let width = f64::from(width) as f32;
            let height = f64::from(height) as f32;
            let options = TerminalOptions {
                keep_last_frame: true,
                ..TerminalOptions::default()
            };
            let (stage, _players) = Stage::open(&title, width, height, PLAYERS, options)
                .map_err(|e| vm.new_runtime_error(e.to_string()))?;
            Ok(PyDisplay {
                inner: PyMutex::new(OnThread::new(stage)),
            })
        }
    }

    #[pyclass(with(Constructor))]
    impl PyDisplay {
        /// Block until the next event, and return it as `(kind, key, alt,
        /// ctrl, shift, meta, repeat)`. Only a key fills the fields after
        /// the kind, and `repeat` is always `False`, because a repeat comes
        /// as another press. A signal that arrives meanwhile runs its handler, so
        /// Ctrl-C raises `KeyboardInterrupt`. A message that the session
        /// drops goes to stderr, and the wait goes on.
        #[pymethod]
        fn wait_event(&self, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
            let event = loop {
                let deadline = Instant::now() + SIGNAL_PERIOD;
                match self.inner.lock().get_mut().wait(Some(deadline)) {
                    Err(Interrupt::Timeout) => vm.check_signals()?,
                    Ok(StageEvent::Error(e)) => eprintln!("spython: {e}"),
                    event => break event,
                }
            };
            let (kind, key) = match event {
                Ok(StageEvent::Tick) => (VSYNC, None),
                Ok(StageEvent::Input { event, .. }) => match event {
                    InputEvent::Key(k) => (k.kind as i32, Some(k)),
                    InputEvent::Resize { .. } => (RESIZE, None),
                    InputEvent::Mouse(_) => (MOUSE, None),
                    InputEvent::Pad(_) => (PAD, None),
                },
                Err(Interrupt::Close) => (CLOSE, None),
                Err(Interrupt::Read(e)) => return Err(vm.new_os_error(e.to_string())),
                // A Wake needs a Sender, and a stage makes none. The loop
                // above takes each Timeout and each Error.
                Ok(StageEvent::Error(_)) | Err(Interrupt::Wake | Interrupt::Timeout) => {
                    unreachable!("the loop above takes every other event")
                }
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
        /// `width` by `height`.
        #[pymethod]
        fn present(
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
                .present(Target::All, scene)
                .map_err(|e| vm.new_runtime_error(e.to_string()))
        }

        /// End the session. A second call does nothing.
        #[pymethod]
        fn close(&self) {
            self.inner.lock().get_mut().close();
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
