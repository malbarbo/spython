//! The `_spython_scene` module: the [`Scene`] of an `Image` of
//! `spython.image`, and the images that a bitmap keeps.

pub(crate) use _spython_scene::{draw, module_def};

#[rustpython_vm::pymodule(name = "_spython_scene")]
pub(crate) mod _spython_scene {
    use rustpython_vm::builtins::PyStrRef;
    use rustpython_vm::function::ArgIntoFloat;
    use rustpython_vm::{
        PyObject, PyObjectRef, PyPayload, PyResult, TryFromObject, VirtualMachine, pyclass,
    };
    use sinteract::scene::{
        Bitmap, Dash, FillRule, FontStyle, Image, LineCap, LineJoin, Paint, Path, PathBuilder,
        PathStyle, RotatedRect, Scene, Text, TextSpec,
    };

    /// The image in `data_uri`, which a `_Bitmap` keeps, so a frame draws it
    /// with no decode. An image larger than the limit of sinteract shrinks.
    #[pyfunction]
    fn load_image(data_uri: PyStrRef, vm: &VirtualMachine) -> PyResult<PyLoadedImage> {
        let blob = data_uri
            .to_str()
            .and_then(crate::bitmap::decode_data_uri)
            .ok_or_else(|| vm.new_value_error("a bitmap needs a data URI in base64"))?;
        let image = Image::load(blob).map_err(|e| vm.new_value_error(e.to_string()))?;
        Ok(PyLoadedImage { image })
    }

    /// An image that [`load_image`] loaded. Python only passes it along.
    #[pyattr]
    #[pyclass(name = "LoadedImage")]
    #[derive(Debug, PyPayload)]
    pub struct PyLoadedImage {
        image: Image,
    }

    #[pyclass]
    impl PyLoadedImage {}

    /// Put `image` into `scene`, as `to_svg` of `spython.image` writes it.
    pub(crate) fn draw(image: &PyObject, scene: &mut Scene, vm: &VirtualMachine) -> PyResult<()> {
        let class = image.class().name().to_owned();
        match class.as_str() {
            "_Path" => {
                if let Some(path) = path(image, vm)? {
                    scene.add_path(path);
                }
            }
            "_Combination" => {
                draw(&*image.get_attr("a", vm)?, scene, vm)?;
                draw(&*image.get_attr("b", vm)?, scene, vm)?;
            }
            "_Crop" => {
                let rect = rotated_rect(&*image.get_attr("box", vm)?, false, false, vm)?;
                let inside = image.get_attr("image", vm)?;
                scene.clip(rect, |clip| draw(&inside, clip, vm))?;
            }
            "_Text" => {
                if let Some(text) = text(image, vm)? {
                    scene.add_text(text);
                }
            }
            "_Bitmap" => {
                let rect = rotated_rect(&*image.get_attr("box", vm)?, false, false, vm)?;
                let loaded = image.get_attr("loaded", vm)?;
                let loaded = loaded
                    .downcast_ref::<PyLoadedImage>()
                    .ok_or_else(|| vm.new_type_error("a bitmap needs the image of load_image"))?;
                scene.add_bitmap(Bitmap::fit(loaded.image.clone(), rect));
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
        text.fill = path_style.fill;
        text.stroke = path_style.stroke;
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
        fill: Option<Color>,
        fill_opacity: f32,
        fill_rule: FillRule,
        stroke: Option<Color>,
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
                None if outline => Color::default(),
                None => Color {
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
                dash: self.dash.as_ref().and_then(|a| Dash::new(a.clone(), 0.0)),
                closed: false,
            }
        }
    }

    /// A `Color` of `spython.color`, with the alpha as a fraction from 0 to
    /// 1.
    #[derive(Clone, Copy, Default)]
    struct Color {
        r: u8,
        g: u8,
        b: u8,
        a: f32,
    }

    /// The color with its alpha times `opacity`, rounded once to the byte of
    /// the scene.
    fn paint(c: Color, opacity: f32) -> Paint {
        let a = (c.a * opacity).clamp(0.0, 1.0) * 255.0;
        Paint::rgba(c.r, c.g, c.b, a.round() as u8)
    }

    /// A `Color` of `spython.color`. `NONE` has no alpha.
    fn color(c: &PyObject, vm: &VirtualMachine) -> PyResult<Color> {
        if c.get_attr("_none", vm)?.try_to_bool(vm)? {
            return Ok(Color::default());
        }
        let channel = |name: &'static str| -> PyResult<u8> {
            let v: i64 = c.get_attr(name, vm)?.try_into_value(vm)?;
            Ok(v.clamp(0, 255) as u8)
        };
        Ok(Color {
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
