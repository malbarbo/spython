//! The `_spython_display` module: a window or a terminal that a World draws
//! into, over [`sinteract::display`], and a printer of images in the
//! terminal. Both walk the tree of an `Image` from `spython.image` and
//! build a [`Scene`], so Python never holds a scene that is half built.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

pub(crate) use _spython_display::module_def;

#[rustpython_vm::pymodule(name = "_spython_display")]
mod _spython_display {
    use std::thread::{self, ThreadId};
    use std::time::{Duration, Instant};

    use rustpython_vm::builtins::{PyStrRef, PyType};
    use rustpython_vm::common::lock::PyMutex;
    use rustpython_vm::function::ArgIntoFloat;
    use rustpython_vm::types::Constructor;
    use rustpython_vm::{
        Py, PyObject, PyObjectRef, PyPayload, PyResult, TryFromObject, VirtualMachine, pyclass,
    };
    use sinteract::asset::Assets;
    use sinteract::display::{Display, PrintError, Printer, TerminalOptions, open_native};
    use sinteract::event::{Event, InputEvent, Interrupt};
    use sinteract::scene::{
        Bitmap, ClipScope, Dash, FillRule, FontStyle, LineCap, LineJoin, Paint, Path, PathBuilder,
        PathStyle, Rgba, RotatedRect, Scene, Text, TextSpec,
    };

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

    /// A printer of images at the cursor, or `None` when the terminal shows
    /// no graphics or stdout is not a terminal.
    #[pyfunction]
    fn open_printer(vm: &VirtualMachine) -> Option<PyObjectRef> {
        let printer = Printer::new().ok()?;
        let printer = PyPrinter {
            inner: PyMutex::new(Printing {
                printer,
                images: Images::default(),
            }),
        };
        Some(printer.into_ref(&vm.ctx).into())
    }

    /// Prints one image after another in the terminal, and keeps the images
    /// of the bitmaps from one print to the next.
    #[pyattr]
    #[pyclass(name = "Printer")]
    #[derive(PyPayload)]
    pub struct PyPrinter {
        inner: PyMutex<Printing>,
    }

    struct Printing {
        printer: Printer,
        images: Images,
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
            let mut inner = self.inner.lock();
            let Printing { printer, images } = &mut *inner;
            draw(&image, &mut scene, images, vm)?;
            images.send(&scene, |id, png| printer.assets_mut().insert(id, png), vm)?;
            match printer.print(&scene) {
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
        inner: PyMutex<OnThread<Box<dyn Display>>>,
        images: PyMutex<Images>,
    }

    impl Constructor for PyDisplay {
        type Args = (String, ArgIntoFloat, ArgIntoFloat);

        /// Open a window of `width` by `height`, and the terminal when no
        /// window opens. The terminal keeps the last frame on screen after
        /// the close.
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
            let display = open_native(&title, width, height, options)
                .map_err(|e| vm.new_runtime_error(e.to_string()))?;
            Ok(PyDisplay {
                inner: PyMutex::new(OnThread::new(display)),
                images: PyMutex::default(),
            })
        }
    }

    #[pyclass(with(Constructor))]
    impl PyDisplay {
        /// Block until the next event, and return it as `(kind, key, alt,
        /// ctrl, shift, meta, repeat)`. Only a key fills the fields after
        /// the kind. A signal that arrives meanwhile runs its handler, so
        /// Ctrl-C raises `KeyboardInterrupt`.
        #[pymethod]
        fn wait_event(&self, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
            let event = loop {
                let deadline = Instant::now() + SIGNAL_PERIOD;
                match self.inner.lock().get_mut().wait_event(Some(deadline)) {
                    Err(Interrupt::Timeout) => vm.check_signals()?,
                    event => break event,
                }
            };
            let (kind, key) = match event {
                Ok(Event::Tick) => (VSYNC, None),
                Ok(Event::Input(InputEvent::Key(k))) => (k.kind as i32, Some(k)),
                Ok(Event::Input(InputEvent::Resize { .. })) => (RESIZE, None),
                Ok(Event::Input(InputEvent::Mouse(_))) => (MOUSE, None),
                Ok(Event::Input(InputEvent::Pad(_))) => (PAD, None),
                Err(Interrupt::Close) => (CLOSE, None),
                Err(Interrupt::Read(e)) => return Err(vm.new_os_error(e.to_string())),
                // A Wake needs a Sender, and this module makes none. The
                // loop above takes each Timeout.
                Err(Interrupt::Wake | Interrupt::Timeout) => {
                    unreachable!("a native display with no Sender wakes only at an event")
                }
            };
            let (name, mods, repeat) = match key {
                Some(k) => (k.key, k.modifiers, k.repeat),
                None => (String::new(), Default::default(), false),
            };
            let elements = vec![
                vm.ctx.new_int(kind).into(),
                vm.ctx.new_str(name).into(),
                vm.ctx.new_bool(mods.alt).into(),
                vm.ctx.new_bool(mods.ctrl).into(),
                vm.ctx.new_bool(mods.shift).into(),
                vm.ctx.new_bool(mods.meta).into(),
                vm.ctx.new_bool(repeat).into(),
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
            let mut images = self.images.lock();
            draw(&image, &mut scene, &mut images, vm)?;
            let mut inner = self.inner.lock();
            let display = inner.get_mut();
            images.send(&scene, |id, png| display.push_asset(id, png), vm)?;
            display
                .present(scene)
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

    /// The images that the bitmaps of a display or of a printer name.
    #[derive(Debug, Default)]
    struct Images {
        assets: Assets,
    }

    impl Images {
        /// The bitmap that draws `data_uri` into `rect`. It takes an id from
        /// the assets at each frame, since an id holds for one frame only.
        fn bitmap(
            &mut self,
            data_uri: &str,
            rect: RotatedRect,
            vm: &VirtualMachine,
        ) -> PyResult<Bitmap> {
            let blob = super::decode_data_uri(data_uri)
                .ok_or_else(|| vm.new_value_error("a bitmap needs a data URI in base64"))?;
            let asset = self
                .assets
                .image(&blob)
                .map_err(|e| vm.new_value_error(e.to_string()))?;
            Ok(asset.fit(rect))
        }

        /// Hand `send` each image of `scene` that has not gone out. An
        /// image that `send` refuses goes out again with the next frame, as
        /// do the ones after it, which never reached `send`.
        fn send<E: std::fmt::Display>(
            &mut self,
            scene: &Scene,
            mut send: impl FnMut(u32, &[u8]) -> Result<(), E>,
            vm: &VirtualMachine,
        ) -> PyResult<()> {
            let uploads = self
                .assets
                .frame(scene)
                .map_err(|e| vm.new_runtime_error(e.to_string()))?;
            let mut uploads = uploads.into_iter();
            while let Some((id, png)) = uploads.next() {
                if let Err(e) = send(id, &png) {
                    self.assets.lost(id);
                    for (id, _) in uploads {
                        self.assets.lost(id);
                    }
                    return Err(vm.new_runtime_error(e.to_string()));
                }
            }
            Ok(())
        }
    }

    /// Where [`draw`] puts an element: the scene, or a clip inside it.
    trait Sink {
        fn add_path(&mut self, path: Path);
        fn text(&mut self, text: Text);
        fn bitmap(&mut self, bitmap: Bitmap);
        fn clip(&mut self, rect: RotatedRect) -> ClipScope<'_>;
    }

    impl Sink for Scene {
        fn add_path(&mut self, path: Path) {
            Scene::add_path(self, path);
        }

        fn text(&mut self, text: Text) {
            Scene::text(self, text);
        }

        fn bitmap(&mut self, bitmap: Bitmap) {
            Scene::bitmap(self, bitmap);
        }

        fn clip(&mut self, rect: RotatedRect) -> ClipScope<'_> {
            Scene::clip(self, rect)
        }
    }

    impl Sink for ClipScope<'_> {
        fn add_path(&mut self, path: Path) {
            ClipScope::add_path(self, path);
        }

        fn text(&mut self, text: Text) {
            ClipScope::text(self, text);
        }

        fn bitmap(&mut self, bitmap: Bitmap) {
            ClipScope::bitmap(self, bitmap);
        }

        fn clip(&mut self, rect: RotatedRect) -> ClipScope<'_> {
            ClipScope::clip(self, rect)
        }
    }

    /// Put `image` into `sink`, as `to_svg` of `spython.image` writes it.
    /// The images of its bitmaps go to `images`.
    fn draw(
        image: &PyObject,
        sink: &mut impl Sink,
        images: &mut Images,
        vm: &VirtualMachine,
    ) -> PyResult<()> {
        let class = image.class().name().to_owned();
        match class.as_str() {
            "_Path" => {
                if let Some(path) = path(image, vm)? {
                    sink.add_path(path);
                }
            }
            "_Combination" => {
                draw(&*image.get_attr("a", vm)?, sink, images, vm)?;
                draw(&*image.get_attr("b", vm)?, sink, images, vm)?;
            }
            "_Crop" => {
                let rect = rotated_rect(&*image.get_attr("box", vm)?, false, false, vm)?;
                let mut clip = sink.clip(rect);
                draw(&*image.get_attr("image", vm)?, &mut clip, images, vm)?;
            }
            "_Text" => {
                if let Some(text) = text(image, vm)? {
                    sink.text(text);
                }
            }
            "_Bitmap" => {
                let rect = rotated_rect(&*image.get_attr("box", vm)?, false, false, vm)?;
                let data_uri = PyStrRef::try_from_object(vm, image.get_attr("data_uri", vm)?)?;
                let data_uri = data_uri
                    .to_str()
                    .ok_or_else(|| vm.new_value_error("a bitmap needs a data URI in base64"))?;
                sink.bitmap(images.bitmap(data_uri, rect, vm)?);
            }
            _ => return Err(vm.new_type_error(format!("cannot draw a {class}"))),
        }
        Ok(())
    }

    /// The path of a `_Path`, or `None` when its commands do not start with
    /// a move, which SVG does not draw either.
    fn path(image: &PyObject, vm: &VirtualMachine) -> PyResult<Option<Path>> {
        let style = Style::read(&*image.get_attr("style", vm)?, vm)?;
        let commands = vm.extract_elements_with(&*image.get_attr("commands", vm)?, Ok)?;
        let Some((first, rest)) = commands.split_first() else {
            return Ok(None);
        };
        if first.class().name().to_owned() != "_MoveTo" {
            return Ok(None);
        }
        // A thin outline on a half pixel covers one row of pixels, and on a
        // whole pixel it blurs over two.
        let snap = |v: f64| {
            if style.aligned() {
                (v + FP_EPSILON).floor() + 0.5
            } else {
                v
            }
        };
        let point = |p: PyObjectRef| -> PyResult<(f32, f32)> {
            Ok((
                snap(float(&p, "x", vm)?) as f32,
                snap(float(&p, "y", vm)?) as f32,
            ))
        };
        let (x, y) = point(first.get_attr("p", vm)?)?;
        let mut path_style = style.path_style();
        path_style.closed = image.get_attr("closed", vm)?.try_to_bool(vm)?;
        let mut builder = Path::builder(path_style, x, y);
        for cmd in rest {
            builder = segment(builder, cmd, &point, vm)?;
        }
        Ok(Some(builder.build()))
    }

    /// `builder` with the segment of `cmd` appended.
    fn segment(
        builder: PathBuilder,
        cmd: &PyObject,
        point: &impl Fn(PyObjectRef) -> PyResult<(f32, f32)>,
        vm: &VirtualMachine,
    ) -> PyResult<PathBuilder> {
        let at = |name: &'static str| point(cmd.get_attr(name, vm)?);
        let class = cmd.class().name().to_owned();
        Ok(match class.as_str() {
            "_MoveTo" => {
                let (x, y) = at("p")?;
                builder.move_to(x, y)
            }
            "_LineTo" => {
                let (x, y) = at("p")?;
                builder.line_to(x, y)
            }
            "_QuadTo" => {
                let (cx, cy) = at("control")?;
                let (x, y) = at("end")?;
                builder.quad_to(cx, cy, x, y)
            }
            "_CubicTo" => {
                let (c1x, c1y) = at("c1")?;
                let (c2x, c2y) = at("c2")?;
                let (x, y) = at("end")?;
                builder.cubic_to(c1x, c1y, c2x, c2y, x, y)
            }
            "_ArcTo" => {
                let (x, y) = at("end")?;
                builder.arc_to(
                    float(cmd, "rx", vm)? as f32,
                    float(cmd, "ry", vm)? as f32,
                    float(cmd, "rotation", vm)? as f32,
                    cmd.get_attr("large_arc", vm)?.try_to_bool(vm)?,
                    cmd.get_attr("sweep", vm)?.try_to_bool(vm)?,
                    x,
                    y,
                )
            }
            _ => return Err(vm.new_type_error(format!("cannot draw a {class}"))),
        })
    }

    /// The `_FP_EPSILON` of `spython.image`.
    const FP_EPSILON: f64 = 1.0e-10;

    /// The text of a `_Text`, or `None` when it draws nothing.
    fn text(image: &PyObject, vm: &VirtualMachine) -> PyResult<Option<Text>> {
        let style = Style::read(&*image.get_attr("style", vm)?, vm)?;
        let font = image.get_attr("font", vm)?;
        let flip_h = image.get_attr("flip_horizontal", vm)?.try_to_bool(vm)?;
        let flip_v = image.get_attr("flip_vertical", vm)?.try_to_bool(vm)?;
        let rect = rotated_rect(&*image.get_attr("box", vm)?, flip_h, flip_v, vm)?;
        let spec = TextSpec {
            size: float(&font, "size", vm)? as f32,
            family: string(&font, "family", vm)?.into(),
            weight: match enum_value(&font, "weight", vm)?.as_str() {
                "lighter" => 300,
                "bold" => 700,
                _ => 400,
            },
            style: match enum_value(&font, "style", vm)?.as_str() {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            },
            text: string(image, "text", vm)?,
        };
        let Some(mut text) = spec.fit(rect) else {
            return Ok(None);
        };
        let path_style = style.path_style();
        text.fill = path_style.fill.primary_color();
        text.stroke = path_style.stroke.primary_color();
        text.stroke_width = path_style.stroke_width;
        text.underline = font.get_attr("underline", vm)?.try_to_bool(vm)?;
        Ok(Some(text))
    }

    /// The rectangle of a `_Box`. A flip mirrors it.
    fn rotated_rect(
        b: &PyObject,
        flip_h: bool,
        flip_v: bool,
        vm: &VirtualMachine,
    ) -> PyResult<RotatedRect> {
        let center = b.get_attr("center", vm)?;
        let sign = |flip: bool| if flip { -1.0 } else { 1.0 };
        Ok(RotatedRect {
            cx: float(&center, "x", vm)? as f32,
            cy: float(&center, "y", vm)? as f32,
            w: (float(b, "width", vm)? * sign(flip_h)) as f32,
            h: (float(b, "height", vm)? * sign(flip_v)) as f32,
            angle_deg: float(b, "angle", vm)? as f32,
        })
    }

    /// A `Style` of `spython.style`, with the defaults that its `to_svg`
    /// adds and that SVG takes for an attribute left out.
    struct Style {
        fill: Option<Rgba>,
        fill_opacity: f32,
        fill_rule: FillRule,
        stroke: Option<Rgba>,
        stroke_width: Option<f32>,
        stroke_opacity: f32,
        dash: Option<Vec<f32>>,
        line_cap: Option<LineCap>,
        line_join: Option<LineJoin>,
    }

    impl Style {
        /// Read the attributes of `style`. Of two that set the same thing,
        /// the first wins, as it does in the SVG of `to_svg` when an HTML
        /// parser reads it.
        fn read(style: &PyObject, vm: &VirtualMachine) -> PyResult<Self> {
            let mut s = Style {
                fill: None,
                fill_opacity: 1.0,
                fill_rule: FillRule::NonZero,
                stroke: None,
                stroke_width: None,
                stroke_opacity: 1.0,
                dash: None,
                line_cap: None,
                line_join: None,
            };
            let mut seen = Vec::new();
            for attr in vm.extract_elements_with(&*style.get_attr("attrs", vm)?, Ok)? {
                let class = attr.class().name().to_owned();
                if seen.contains(&class) {
                    continue;
                }
                match class.as_str() {
                    "_Fill" => s.fill = Some(color(&*attr.get_attr("color", vm)?, vm)?),
                    "_FillOpacity" => s.fill_opacity = float(&attr, "value", vm)? as f32,
                    "_FillRule" => {
                        if enum_value(&attr, "value", vm)? == "evenodd" {
                            s.fill_rule = FillRule::EvenOdd;
                        }
                    }
                    "_Stroke" => s.stroke = Some(color(&*attr.get_attr("color", vm)?, vm)?),
                    "_StrokeWidth" => s.stroke_width = Some(float(&attr, "value", vm)? as f32),
                    "_StrokeOpacity" => s.stroke_opacity = float(&attr, "value", vm)? as f32,
                    "_StrokeDashArray" => {
                        let values = vm
                            .extract_elements_with(&*attr.get_attr("values", vm)?, |v| {
                                Ok(f64::from(ArgIntoFloat::try_from_object(vm, v)?) as f32)
                            })?;
                        s.dash = Some(values);
                    }
                    "_StrokeLineCap" => {
                        s.line_cap = Some(match enum_value(&attr, "value", vm)?.as_str() {
                            "round" => LineCap::Round,
                            "square" => LineCap::Square,
                            _ => LineCap::Butt,
                        });
                    }
                    "_StrokeLineJoin" => {
                        s.line_join = Some(match enum_value(&attr, "value", vm)?.as_str() {
                            "round" => LineJoin::Round,
                            "bevel" => LineJoin::Bevel,
                            _ => LineJoin::Miter,
                        });
                    }
                    _ => return Err(vm.new_type_error(format!("unknown style {class}"))),
                }
                seen.push(class);
            }
            Ok(s)
        }

        /// Returns `true` if the path draws a thin outline on half pixels,
        /// `false` otherwise. It is the `outline_offset` of `spython.style`.
        fn aligned(&self) -> bool {
            self.stroke.is_some() && self.fill.is_none() && self.stroke_width.is_none()
        }

        fn path_style(&self) -> PathStyle {
            let outline = self.stroke.is_some();
            // SVG fills black when no fill is set, and to_svg sets none
            // for an outline.
            let fill = match self.fill {
                Some(c) => c,
                None if outline => Rgba::default(),
                None => Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 1.0,
                },
            };
            let stroke = self.stroke.unwrap_or_default();
            PathStyle {
                fill: paint(fill, self.fill_opacity),
                stroke: paint(stroke, self.stroke_opacity),
                stroke_width: if outline {
                    self.stroke_width.unwrap_or(1.0)
                } else {
                    0.0
                },
                line_cap: self.line_cap.unwrap_or(if outline {
                    LineCap::Round
                } else {
                    LineCap::Butt
                }),
                line_join: self.line_join.unwrap_or(LineJoin::Miter),
                fill_rule: self.fill_rule,
                // to_svg writes a limit of 10 only with the join it adds.
                miter_limit: if self.line_join.is_none() && outline {
                    10.0
                } else {
                    sinteract::scene::DEFAULT_MITER_LIMIT
                },
                dash: self
                    .dash
                    .as_ref()
                    .and_then(|a| Dash::new(a.clone(), 0.0))
                    .map(Box::new),
                closed: false,
            }
        }
    }

    fn paint(c: Rgba, opacity: f32) -> Paint {
        Paint::rgba(c.r, c.g, c.b, c.a * opacity)
    }

    /// A `Color` of `spython.color`. `NONE` has no alpha.
    fn color(c: &PyObject, vm: &VirtualMachine) -> PyResult<Rgba> {
        if c.get_attr("_none", vm)?.try_to_bool(vm)? {
            return Ok(Rgba::default());
        }
        let channel = |name: &'static str| -> PyResult<u8> {
            let v = float(c, name, vm)?;
            Ok(v.clamp(0.0, 255.0) as u8)
        };
        Ok(Rgba {
            r: channel("r")?,
            g: channel("g")?,
            b: channel("b")?,
            a: float(c, "a", vm)? as f32,
        })
    }

    fn float(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<f64> {
        Ok(ArgIntoFloat::try_from_object(vm, obj.get_attr(name, vm)?)?.into())
    }

    fn string(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<String> {
        String::try_from_object(vm, obj.get_attr(name, vm)?)
    }

    /// The `value` of the `Enum` at `obj.name`.
    fn enum_value(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<String> {
        string(&*obj.get_attr(name, vm)?, "value", vm)
    }
}

/// The bytes of a `data:` URI in base64, or `None` when `uri` is not one.
fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let (header, payload) = uri.strip_prefix("data:")?.split_once(',')?;
    header.ends_with(";base64").then_some(())?;
    B64.decode(payload).ok()
}

/// The width and the height of the PNG in `blob`, from its header, or `None`
/// when `blob` is not a PNG.
pub(crate) fn png_size(blob: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    // The IHDR chunk comes first, with the width and the height at the start
    // of its data.
    let rest = blob.strip_prefix(SIGNATURE)?;
    let (_length, rest) = rest.split_first_chunk::<4>()?;
    let rest = rest.strip_prefix(b"IHDR")?;
    let (width, rest) = rest.split_first_chunk::<4>()?;
    let (height, _) = rest.split_first_chunk::<4>()?;
    Some((u32::from_be_bytes(*width), u32::from_be_bytes(*height)))
}

/// A data URI that holds the PNG in `blob`.
pub(crate) fn png_data_uri(blob: &[u8]) -> String {
    format!("data:image/png;base64,{}", B64.encode(blob))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signature and the start of the IHDR chunk of a PNG of 3 by 2.
    const HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x03\0\0\0\x02";

    #[test]
    fn png_size_reads_the_header() {
        assert_eq!(png_size(HEADER), Some((3, 2)));
    }

    #[test]
    fn png_size_refuses_what_is_not_a_png() {
        assert_eq!(
            png_size(b"GIF89a\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
            None
        );
        assert_eq!(png_size(HEADER.get(..20).expect("a prefix")), None);
    }

    #[test]
    fn a_data_uri_decodes_back_to_its_bytes() {
        assert_eq!(
            decode_data_uri(&png_data_uri(HEADER)).as_deref(),
            Some(HEADER)
        );
    }

    #[test]
    fn decode_data_uri_refuses_a_uri_that_is_not_base64() {
        assert_eq!(decode_data_uri("data:image/png,abc"), None);
        assert_eq!(decode_data_uri("image/png;base64,abc"), None);
    }
}
