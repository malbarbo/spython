//! The `_spython_image` module: the `Image` of `spython.image` over
//! [`sinteract::image`]. Each function converts its arguments into the types
//! of sinteract, and an argument that breaks a rule raises `ValueError` with
//! the name of the argument and the value as the program wrote it.

pub(crate) use _spython_image::{PyImage, module_def};

#[rustpython_vm::pymodule(name = "_spython_image")]
pub(crate) mod _spython_image {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use num_traits::Signed;
    use rustpython_vm::builtins::{PyBaseExceptionRef, PyType};
    use rustpython_vm::function::{ArgIntoFloat, OptionalArg};
    use rustpython_vm::types::Constructor;
    use rustpython_vm::{
        FromArgs, Py, PyObject, PyObjectRef, PyPayload, PyRef, PyResult, TryFromObject,
        VirtualMachine, pyclass,
    };
    use sinteract::image::{
        AngleSumError, AtLeast, Attribute, BitmapError, Color, CurveEnd, Factor, FactorError,
        Finite, Font, FontWeight, Image, Length, LengthError, Opacity, Overflow, PathCmd, Point,
        StarPolygonError, Style, TooLarge, TriangleAngle, TriangleSidesError, WedgeAngle,
        WedgeAngleError, XPlace, YPlace,
    };
    use sinteract::scene::{FillRule, FontStyle, LineCap, LineJoin};

    #[pyattr]
    #[pyclass(module = "spython.image", name = "Image")]
    #[derive(Debug, PyPayload)]
    pub struct PyImage(pub(crate) Image);

    #[pyclass(flags(DISALLOW_INSTANTIATION))]
    impl PyImage {}

    #[pyattr]
    #[pyclass(module = "spython.image", name = "_PathCmd")]
    #[derive(Debug, PyPayload)]
    pub struct PyPathCmd(PathCmd);

    #[pyclass(flags(DISALLOW_INSTANTIATION))]
    impl PyPathCmd {}

    type ImageRef = PyRef<PyImage>;

    /// The `ValueError` of the argument `name`, whose value `value` breaks
    /// `rule`.
    fn error(name: &str, rule: &str, value: &PyObject, vm: &VirtualMachine) -> PyBaseExceptionRef {
        match value.str(vm) {
            Ok(got) => vm.new_value_error(format!("{name} must {rule}, got {got}")),
            Err(e) => e,
        }
    }

    fn overflow(_: Overflow, vm: &VirtualMachine) -> PyBaseExceptionRef {
        vm.new_value_error(Overflow.to_string())
    }

    fn too_large(_: TooLarge, vm: &VirtualMachine) -> PyBaseExceptionRef {
        vm.new_value_error(TooLarge.to_string())
    }

    fn image(image: Result<Image, Overflow>, vm: &VirtualMachine) -> PyResult<PyImage> {
        image.map(PyImage).map_err(|e| overflow(e, vm))
    }

    fn float(value: &PyObject, vm: &VirtualMachine) -> PyResult<f64> {
        Ok(ArgIntoFloat::try_from_object(vm, value.to_owned())?.into())
    }

    fn finite(name: &str, value: &PyObject, vm: &VirtualMachine) -> PyResult<Finite> {
        Finite::new(float(value, vm)?).map_err(|_| error(name, "be a finite number", value, vm))
    }

    fn length(name: &str, value: &PyObject, vm: &VirtualMachine) -> PyResult<Length> {
        Length::new(float(value, vm)?).map_err(|e| {
            let rule = match e {
                LengthError::NotFinite => "be a finite number",
                LengthError::Negative => "not be negative",
            };
            error(name, rule, value, vm)
        })
    }

    fn factor(name: &str, value: &PyObject, vm: &VirtualMachine) -> PyResult<Factor> {
        Factor::new(float(value, vm)?).map_err(|e| {
            let rule = match e {
                FactorError::NotFinite => "be a finite number",
                FactorError::NotPositive => "be positive",
            };
            error(name, rule, value, vm)
        })
    }

    fn opacity(name: &str, value: &PyObject, vm: &VirtualMachine) -> PyResult<Opacity> {
        Opacity::new(float(value, vm)?).map_err(|_| error(name, "be between 0 and 1", value, vm))
    }

    fn triangle_angle(
        name: &str,
        value: &PyObject,
        vm: &VirtualMachine,
    ) -> PyResult<TriangleAngle> {
        TriangleAngle::new(float(value, vm)?)
            .map_err(|_| error(name, "be greater than 0 and less than 180", value, vm))
    }

    fn wedge_angle(value: &PyObject, vm: &VirtualMachine) -> PyResult<WedgeAngle> {
        WedgeAngle::new(float(value, vm)?).map_err(|e| {
            let rule = match e {
                WedgeAngleError::NotFinite => "be a finite number",
                WedgeAngleError::Zero => "not be 0",
                WedgeAngleError::OutOfRange => "be between -360 and 360",
            };
            error("angle", rule, value, vm)
        })
    }

    /// The most sides or points a shape takes. sinteract allocates them all
    /// at once, and 10^6 of them already take seconds to draw.
    const MAX_COUNT: u32 = 100_000;

    fn count<const MIN: u32>(
        name: &str,
        value: &PyObject,
        max: u32,
        vm: &VirtualMachine,
    ) -> PyResult<AtLeast<MIN>> {
        let too_many = || error(name, &format!("be at most {max}"), value, vm);
        let index = value.try_index(vm)?;
        let n = match u32::try_from(index.as_bigint()) {
            Ok(n) if n <= max => n,
            // A negative count is too few, as 0 is.
            Err(_) if index.as_bigint().is_negative() => 0,
            _ => return Err(too_many()),
        };
        AtLeast::new(n).map_err(|_| error(name, &format!("be at least {MIN}"), value, vm))
    }

    fn point(
        x: &PyObject,
        y: &PyObject,
        names: (&str, &str),
        vm: &VirtualMachine,
    ) -> PyResult<Point> {
        Ok(Point::new(finite(names.0, x, vm)?, finite(names.1, y, vm)?))
    }

    /// The `N` elements of the tuple `obj`.
    fn elements<const N: usize>(obj: &PyObject, vm: &VirtualMachine) -> PyResult<[PyObjectRef; N]> {
        let elements = vm.extract_elements_with(obj, Ok)?;
        <[PyObjectRef; N]>::try_from(elements)
            .map_err(|_| vm.new_type_error(format!("expected a tuple of {N} elements")))
    }

    /// The points of `points`, each a tuple `(x, y)`.
    fn points(points: Vec<PyObjectRef>, vm: &VirtualMachine) -> PyResult<Vec<Point>> {
        points
            .iter()
            .map(|p| {
                let [x, y] = elements(p, vm)?;
                point(&x, &y, ("x", "y"), vm)
            })
            .collect()
    }

    /// The end of a curve in the tuple `(x, y, angle, pull)`, with the names
    /// of its arguments ending in `n`.
    fn curve_end(end: PyObjectRef, n: u8, vm: &VirtualMachine) -> PyResult<CurveEnd> {
        let [x, y, angle, pull] = elements(&end, vm)?;
        Ok(CurveEnd {
            point: point(&x, &y, (&format!("x{n}"), &format!("y{n}")), vm)?,
            angle: finite(&format!("angle{n}"), &angle, vm)?,
            pull: finite(&format!("pull{n}"), &pull, vm)?,
        })
    }

    fn x_place(place: &PyObject, vm: &VirtualMachine) -> PyResult<XPlace> {
        Ok(match enum_value(place, vm)?.as_str() {
            "left" => XPlace::Left,
            "right" => XPlace::Right,
            _ => XPlace::Center,
        })
    }

    fn y_place(place: &PyObject, vm: &VirtualMachine) -> PyResult<YPlace> {
        Ok(match enum_value(place, vm)?.as_str() {
            "top" => YPlace::Top,
            "bottom" => YPlace::Bottom,
            _ => YPlace::Middle,
        })
    }

    fn images(images: Vec<ImageRef>) -> impl Iterator<Item = Image> {
        images.into_iter().map(|i| i.0.clone())
    }

    /// The `value` of an `Enum` of `spython`.
    fn enum_value(e: &PyObject, vm: &VirtualMachine) -> PyResult<String> {
        String::try_from_object(vm, e.get_attr("value", vm)?)
    }

    /// The member of the `Enum` `name` of `spython.font` with `value`.
    fn font_enum(name: &'static str, value: &str, vm: &VirtualMachine) -> PyResult {
        let module = vm.import("spython.font", 0)?;
        module.get_attr(name, vm)?.call((value.to_owned(),), vm)
    }

    // Color

    #[pyattr]
    #[pyclass(module = "spython.color", name = "Color")]
    #[derive(Debug, PyPayload)]
    pub struct PyColor(Color);

    #[derive(FromArgs)]
    pub struct ColorArgs {
        #[pyarg(any)]
        r: PyObjectRef,
        #[pyarg(any)]
        g: PyObjectRef,
        #[pyarg(any)]
        b: PyObjectRef,
        #[pyarg(any, optional)]
        a: OptionalArg<PyObjectRef>,
    }

    impl Constructor for PyColor {
        type Args = ColorArgs;

        fn py_new(_cls: &Py<PyType>, args: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            let channel = |name: &str, v: &PyObject| -> PyResult<u8> {
                let n = v.try_index(vm)?;
                u8::try_from(n.as_bigint()).map_err(|_| error(name, "be between 0 and 255", v, vm))
            };
            let (r, g, b) = (
                channel("r", &args.r)?,
                channel("g", &args.g)?,
                channel("b", &args.b)?,
            );
            let a = match args.a {
                OptionalArg::Present(a) => opacity("a", &a, vm)?,
                OptionalArg::Missing => Opacity::OPAQUE,
            };
            Ok(PyColor(Color::rgba(r, g, b, a)))
        }
    }

    #[pyclass(with(Constructor))]
    impl PyColor {
        #[pygetset]
        fn r(&self) -> u8 {
            self.0.r
        }

        #[pygetset]
        fn g(&self) -> u8 {
            self.0.g
        }

        #[pygetset]
        fn b(&self) -> u8 {
            self.0.b
        }

        #[pygetset]
        fn a(&self) -> f64 {
            self.0.a.get()
        }
    }

    type ColorRef = PyRef<PyColor>;

    // Font

    #[pyattr]
    #[pyclass(module = "spython.font", name = "Font")]
    #[derive(Debug, PyPayload)]
    pub struct PyFont(Font);

    #[derive(FromArgs)]
    pub struct FontArgs {
        #[pyarg(named, optional)]
        family: OptionalArg<String>,
        #[pyarg(named, optional)]
        size: OptionalArg<PyObjectRef>,
        #[pyarg(named, optional)]
        style: OptionalArg<PyObjectRef>,
        #[pyarg(named, optional)]
        weight: OptionalArg<PyObjectRef>,
        #[pyarg(named, optional)]
        underline: OptionalArg<bool>,
    }

    impl Constructor for PyFont {
        type Args = FontArgs;

        fn py_new(_cls: &Py<PyType>, args: Self::Args, vm: &VirtualMachine) -> PyResult<Self> {
            let mut font = Font::default();
            if let OptionalArg::Present(family) = args.family {
                font.family = family.into();
            }
            if let OptionalArg::Present(size) = args.size {
                font.size = length("size", &size, vm)?;
            }
            if let OptionalArg::Present(style) = args.style {
                font.style = match enum_value(&style, vm)?.as_str() {
                    "italic" => FontStyle::Italic,
                    "oblique" => FontStyle::Oblique,
                    _ => FontStyle::Normal,
                };
            }
            if let OptionalArg::Present(weight) = args.weight {
                font.weight = match enum_value(&weight, vm)?.as_str() {
                    "lighter" => FontWeight::Light,
                    "bold" => FontWeight::Bold,
                    _ => FontWeight::Regular,
                };
            }
            if let OptionalArg::Present(underline) = args.underline {
                font.underline = underline;
            }
            Ok(PyFont(font))
        }
    }

    #[pyclass(with(Constructor))]
    impl PyFont {
        #[pygetset]
        fn family(&self) -> String {
            self.0.family.to_string()
        }

        #[pygetset]
        fn size(&self) -> f64 {
            self.0.size.get()
        }

        #[pygetset]
        fn style(&self, vm: &VirtualMachine) -> PyResult {
            let value = match self.0.style {
                FontStyle::Normal => "normal",
                FontStyle::Italic => "italic",
                FontStyle::Oblique => "oblique",
            };
            font_enum("FontStyle", value, vm)
        }

        #[pygetset]
        fn weight(&self, vm: &VirtualMachine) -> PyResult {
            let value = match self.0.weight {
                FontWeight::Light => "lighter",
                FontWeight::Regular => "normal",
                FontWeight::Bold => "bold",
            };
            font_enum("FontWeight", value, vm)
        }

        #[pygetset]
        fn underline(&self) -> bool {
            self.0.underline
        }
    }

    // Style

    #[pyattr]
    #[pyclass(module = "spython.style", name = "Style")]
    #[derive(Debug, PyPayload)]
    pub struct PyStyle(Style);

    #[pyclass(flags(DISALLOW_INSTANTIATION))]
    impl PyStyle {}

    type StyleRef = PyRef<PyStyle>;

    #[pyfunction]
    fn join(styles: Vec<StyleRef>) -> PyStyle {
        PyStyle(Style::join(styles.iter().map(|s| &s.0)))
    }

    #[pyfunction]
    fn fill(
        c: ColorRef,
        opacity: Option<PyObjectRef>,
        rule: Option<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyStyle> {
        let mut attributes = vec![Attribute::Fill(c.0)];
        if let Some(v) = opacity {
            attributes.push(Attribute::FillOpacity(self::opacity("opacity", &v, vm)?));
        }
        if let Some(rule) = rule {
            attributes.push(Attribute::FillRule(match enum_value(&rule, vm)?.as_str() {
                "evenodd" => FillRule::EvenOdd,
                _ => FillRule::NonZero,
            }));
        }
        Ok(PyStyle(Style::new(attributes)))
    }

    #[pyfunction]
    fn stroke(
        c: ColorRef,
        width: Option<PyObjectRef>,
        opacity: Option<PyObjectRef>,
        dash_array: Option<Vec<PyObjectRef>>,
        linecap: Option<PyObjectRef>,
        linejoin: Option<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyStyle> {
        let mut attributes = vec![Attribute::Stroke(c.0)];
        if let Some(v) = width {
            attributes.push(Attribute::StrokeWidth(length("width", &v, vm)?));
        }
        if let Some(v) = opacity {
            attributes.push(Attribute::StrokeOpacity(self::opacity("opacity", &v, vm)?));
        }
        if let Some(values) = dash_array {
            let values = values.iter().map(|v| length("dash_array", v, vm));
            attributes.push(Attribute::StrokeDashArray(values.collect::<PyResult<_>>()?));
        }
        if let Some(cap) = linecap {
            attributes.push(Attribute::StrokeLineCap(
                match enum_value(&cap, vm)?.as_str() {
                    "round" => LineCap::Round,
                    "square" => LineCap::Square,
                    _ => LineCap::Butt,
                },
            ));
        }
        if let Some(join) = linejoin {
            attributes.push(Attribute::StrokeLineJoin(
                match enum_value(&join, vm)?.as_str() {
                    "round" => LineJoin::Round,
                    "bevel" => LineJoin::Bevel,
                    _ => LineJoin::Miter,
                },
            ));
        }
        Ok(PyStyle(Style::new(attributes)))
    }

    // Path

    #[pyfunction]
    fn move_to(x: PyObjectRef, y: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyPathCmd> {
        Ok(PyPathCmd(PathCmd::move_to(point(&x, &y, ("x", "y"), vm)?)))
    }

    #[pyfunction]
    fn line_to(x: PyObjectRef, y: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyPathCmd> {
        Ok(PyPathCmd(PathCmd::line_to(point(&x, &y, ("x", "y"), vm)?)))
    }

    #[pyfunction]
    fn quad_to(
        cx: PyObjectRef,
        cy: PyObjectRef,
        x: PyObjectRef,
        y: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyPathCmd> {
        let control = point(&cx, &cy, ("cx", "cy"), vm)?;
        let end = point(&x, &y, ("x", "y"), vm)?;
        Ok(PyPathCmd(PathCmd::quad_to(control, end)))
    }

    #[pyfunction]
    fn cubic_to(
        c1x: PyObjectRef,
        c1y: PyObjectRef,
        c2x: PyObjectRef,
        c2y: PyObjectRef,
        x: PyObjectRef,
        y: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyPathCmd> {
        let c1 = point(&c1x, &c1y, ("c1x", "c1y"), vm)?;
        let c2 = point(&c2x, &c2y, ("c2x", "c2y"), vm)?;
        let end = point(&x, &y, ("x", "y"), vm)?;
        Ok(PyPathCmd(PathCmd::cubic_to(c1, c2, end)))
    }

    #[pyfunction]
    fn arc_to(
        rx: PyObjectRef,
        ry: PyObjectRef,
        rotation: PyObjectRef,
        large_arc: bool,
        sweep: bool,
        end: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyPathCmd> {
        let rx = finite("rx", &rx, vm)?;
        let ry = finite("ry", &ry, vm)?;
        let rotation = finite("rotation", &rotation, vm)?;
        let [x, y] = elements(&end, vm)?;
        let end = point(&x, &y, ("x", "y"), vm)?;
        Ok(PyPathCmd(PathCmd::arc_to(
            rx, ry, rotation, large_arc, sweep, end,
        )))
    }

    #[pyfunction]
    fn path(
        commands: Vec<PyRef<PyPathCmd>>,
        closed: bool,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let commands: Vec<PathCmd> = commands.iter().map(|c| c.0).collect();
        image(Image::path(commands, closed, s.0.clone()), vm)
    }

    // Shapes

    #[pyfunction]
    fn empty() -> PyImage {
        PyImage(Image::empty())
    }

    #[pyfunction]
    fn rectangle(
        w: PyObjectRef,
        h: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (w, h) = (length("w", &w, vm)?, length("h", &h, vm)?);
        Ok(PyImage(Image::rectangle(w, h, s.0.clone())))
    }

    #[pyfunction]
    fn square(side: PyObjectRef, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let side = length("side", &side, vm)?;
        Ok(PyImage(Image::square(side, s.0.clone())))
    }

    #[pyfunction]
    fn ellipse(
        w: PyObjectRef,
        h: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (w, h) = (length("w", &w, vm)?, length("h", &h, vm)?);
        image(Image::ellipse(w, h, s.0.clone()), vm)
    }

    #[pyfunction]
    fn circle(radius: PyObjectRef, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let radius = length("radius", &radius, vm)?;
        image(Image::circle(radius, s.0.clone()), vm)
    }

    #[pyfunction]
    fn line(x: PyObjectRef, y: PyObjectRef, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let (x, y) = (finite("x", &x, vm)?, finite("y", &y, vm)?);
        Ok(PyImage(Image::line(x, y, s.0.clone())))
    }

    #[pyfunction]
    fn triangle(side: PyObjectRef, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let side = length("side", &side, vm)?;
        Ok(PyImage(Image::triangle(side, s.0.clone())))
    }

    #[pyfunction]
    fn right_triangle(
        side1: PyObjectRef,
        side2: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (side1, side2) = (length("side1", &side1, vm)?, length("side2", &side2, vm)?);
        Ok(PyImage(Image::right_triangle(side1, side2, s.0.clone())))
    }

    #[pyfunction]
    fn isosceles_triangle(
        side_length: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        let angle = finite("angle", &angle, vm)?;
        image(Image::isosceles_triangle(side, angle, s.0.clone()), vm)
    }

    #[pyfunction]
    fn triangle_sss(
        side_a: PyObjectRef,
        side_b: PyObjectRef,
        side_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = length("side_a", &side_a, vm)?;
        let b = length("side_b", &side_b, vm)?;
        let c = length("side_c", &side_c, vm)?;
        Image::triangle_sss(a, b, c, s.0.clone())
            .map(PyImage)
            .map_err(|e| match e {
                TriangleSidesError::TooLong => {
                    let shown = |v: &PyObject| v.str(vm).map(|s| s.to_string());
                    match (shown(&side_a), shown(&side_b), shown(&side_c)) {
                        (Ok(a), Ok(b), Ok(c)) => vm.new_value_error(format!(
                            "no side can be longer than the sum of the other two, \
                             got {a}, {b} and {c}"
                        )),
                        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => e,
                    }
                }
                TriangleSidesError::Overflow => overflow(Overflow, vm),
            })
    }

    #[pyfunction]
    fn triangle_sas(
        side_a: PyObjectRef,
        angle_b: PyObjectRef,
        side_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = length("side_a", &side_a, vm)?;
        let angle_b = triangle_angle("angle_b", &angle_b, vm)?;
        let c = length("side_c", &side_c, vm)?;
        image(Image::triangle_sas(a, angle_b, c, s.0.clone()), vm)
    }

    #[pyfunction]
    fn triangle_ssa(
        side_a: PyObjectRef,
        side_b: PyObjectRef,
        angle_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = length("side_a", &side_a, vm)?;
        let b = length("side_b", &side_b, vm)?;
        let angle_c = triangle_angle("angle_c", &angle_c, vm)?;
        image(Image::triangle_ssa(a, b, angle_c, s.0.clone()), vm)
    }

    #[pyfunction]
    fn triangle_ass(
        angle_a: PyObjectRef,
        side_b: PyObjectRef,
        side_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let angle_a = triangle_angle("angle_a", &angle_a, vm)?;
        let b = length("side_b", &side_b, vm)?;
        let c = length("side_c", &side_c, vm)?;
        image(Image::triangle_ass(angle_a, b, c, s.0.clone()), vm)
    }

    /// The error of a triangle from the two angles `first` and `second`.
    fn angle_sum(
        e: AngleSumError,
        first: (&str, &PyObject),
        second: (&str, &PyObject),
        vm: &VirtualMachine,
    ) -> PyBaseExceptionRef {
        match e {
            AngleSumError::AtLeast180 => {
                let name = format!("{} + {}", first.0, second.0);
                match vm._add(first.1, second.1) {
                    Ok(sum) => error(&name, "be less than 180", &sum, vm),
                    Err(e) => e,
                }
            }
            AngleSumError::Overflow => overflow(Overflow, vm),
        }
    }

    #[pyfunction]
    fn triangle_aas(
        angle_a: PyObjectRef,
        angle_b: PyObjectRef,
        side_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = triangle_angle("angle_a", &angle_a, vm)?;
        let b = triangle_angle("angle_b", &angle_b, vm)?;
        let c = length("side_c", &side_c, vm)?;
        Image::triangle_aas(a, b, c, s.0.clone())
            .map(PyImage)
            .map_err(|e| angle_sum(e, ("angle_a", &angle_a), ("angle_b", &angle_b), vm))
    }

    #[pyfunction]
    fn triangle_asa(
        angle_a: PyObjectRef,
        side_b: PyObjectRef,
        angle_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = triangle_angle("angle_a", &angle_a, vm)?;
        let b = length("side_b", &side_b, vm)?;
        let c = triangle_angle("angle_c", &angle_c, vm)?;
        Image::triangle_asa(a, b, c, s.0.clone())
            .map(PyImage)
            .map_err(|e| angle_sum(e, ("angle_a", &angle_a), ("angle_c", &angle_c), vm))
    }

    #[pyfunction]
    fn triangle_saa(
        side_a: PyObjectRef,
        angle_b: PyObjectRef,
        angle_c: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let a = length("side_a", &side_a, vm)?;
        let b = triangle_angle("angle_b", &angle_b, vm)?;
        let c = triangle_angle("angle_c", &angle_c, vm)?;
        Image::triangle_saa(a, b, c, s.0.clone())
            .map(PyImage)
            .map_err(|e| angle_sum(e, ("angle_b", &angle_b), ("angle_c", &angle_c), vm))
    }

    #[pyfunction]
    fn rhombus(
        side_length: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        let angle = finite("angle", &angle, vm)?;
        image(Image::rhombus(side, angle, s.0.clone()), vm)
    }

    #[pyfunction]
    fn regular_polygon(
        side_length: PyObjectRef,
        side_count: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        let n = count("side_count", &side_count, MAX_COUNT, vm)?;
        image(Image::regular_polygon(side, n, s.0.clone()), vm)
    }

    #[pyfunction]
    fn polygon(pts: Vec<PyObjectRef>, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let pts = points(pts, vm)?;
        image(Image::polygon(&pts, s.0.clone()), vm)
    }

    #[pyfunction]
    fn star_polygon(
        side_length: PyObjectRef,
        side_count: PyObjectRef,
        step_count: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        let n = count("side_count", &side_count, MAX_COUNT, vm)?;
        let step = count("step_count", &step_count, u32::MAX, vm)?;
        Image::star_polygon(side, n, step, s.0.clone())
            .map(PyImage)
            .map_err(|e| match e {
                StarPolygonError::CommonDivisor => vm.new_value_error(format!(
                    "side_count and step_count must have no common divisor, got {} and {}",
                    n.get(),
                    step.get()
                )),
                StarPolygonError::Overflow => overflow(Overflow, vm),
            })
    }

    #[pyfunction]
    fn star(side_length: PyObjectRef, s: StyleRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        image(Image::star(side, s.0.clone()), vm)
    }

    #[pyfunction]
    fn radial_star(
        point_count: PyObjectRef,
        inner_radius: PyObjectRef,
        outer_radius: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let n = count("point_count", &point_count, MAX_COUNT, vm)?;
        let inner = length("inner_radius", &inner_radius, vm)?;
        let outer = length("outer_radius", &outer_radius, vm)?;
        image(Image::radial_star(n, inner, outer, s.0.clone()), vm)
    }

    #[pyfunction]
    fn pulled_regular_polygon(
        side_length: PyObjectRef,
        side_count: PyObjectRef,
        pull: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let side = length("side_length", &side_length, vm)?;
        let n = count("side_count", &side_count, MAX_COUNT, vm)?;
        let pull = finite("pull", &pull, vm)?;
        let angle = finite("angle", &angle, vm)?;
        image(
            Image::pulled_regular_polygon(side, n, pull, angle, s.0.clone()),
            vm,
        )
    }

    #[pyfunction]
    fn wedge(
        radius: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let radius = length("radius", &radius, vm)?;
        let angle = wedge_angle(&angle, vm)?;
        image(Image::wedge(radius, angle, s.0.clone()), vm)
    }

    #[pyfunction]
    fn text(
        txt: String,
        f: PyRef<PyFont>,
        size: Option<PyObjectRef>,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let mut font = f.0.clone();
        if let Some(size) = size {
            font.size = length("size", &size, vm)?;
        }
        Ok(PyImage(Image::text(&txt, font, s.0.clone())))
    }

    #[pyfunction]
    fn bitmap_data_uri(data_uri: String, vm: &VirtualMachine) -> PyResult<PyImage> {
        Image::bitmap_data_uri(&data_uri)
            .map(PyImage)
            .map_err(|e| match e {
                BitmapError::NotDataUri => {
                    vm.new_value_error("a bitmap needs a data URI in base64")
                }
                BitmapError::Image(e) => vm.new_value_error(e.to_string()),
            })
    }

    // Measures

    #[pyfunction]
    fn width(img: ImageRef) -> f64 {
        img.0.width().get()
    }

    #[pyfunction]
    fn height(img: ImageRef) -> f64 {
        img.0.height().get()
    }

    #[pyfunction]
    fn center(img: ImageRef) -> (f64, f64) {
        let c = img.0.center();
        (c.x(), c.y())
    }

    // Transformations

    #[pyfunction]
    fn rotate(img: ImageRef, angle: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let angle = finite("angle", &angle, vm)?;
        image(img.0.rotate(angle), vm)
    }

    #[pyfunction]
    fn scale(img: ImageRef, f: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        let f = factor("factor", &f, vm)?;
        image(img.0.scale(f), vm)
    }

    #[pyfunction]
    fn scale_xy(
        img: ImageRef,
        x_factor: PyObjectRef,
        y_factor: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let x = factor("x_factor", &x_factor, vm)?;
        let y = factor("y_factor", &y_factor, vm)?;
        image(img.0.scale_xy(x, y), vm)
    }

    #[pyfunction]
    fn flip_horizontal(img: ImageRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        image(img.0.flip_horizontal(), vm)
    }

    #[pyfunction]
    fn flip_vertical(img: ImageRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        image(img.0.flip_vertical(), vm)
    }

    #[pyfunction]
    fn frame(img: ImageRef, c: ColorRef) -> PyImage {
        PyImage(img.0.frame(c.0))
    }

    #[pyfunction]
    fn crop(
        img: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        w: PyObjectRef,
        h: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (x, y) = (finite("x", &x, vm)?, finite("y", &y, vm)?);
        let (w, h) = (length("w", &w, vm)?, length("h", &h, vm)?);
        Ok(PyImage(img.0.crop(x, y, w, h)))
    }

    #[pyfunction]
    fn crop_align(
        img: ImageRef,
        xp: PyObjectRef,
        yp: PyObjectRef,
        cw: PyObjectRef,
        ch: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (xp, yp) = (x_place(&xp, vm)?, y_place(&yp, vm)?);
        let (w, h) = (length("cw", &cw, vm)?, length("ch", &ch, vm)?);
        Ok(PyImage(img.0.crop_align(xp, yp, w, h)))
    }

    // Composition

    #[pyfunction]
    fn above(imgs: Vec<ImageRef>, xp: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        image(Image::above(x_place(&xp, vm)?, images(imgs)), vm)
    }

    #[pyfunction]
    fn beside(imgs: Vec<ImageRef>, yp: PyObjectRef, vm: &VirtualMachine) -> PyResult<PyImage> {
        image(Image::beside(y_place(&yp, vm)?, images(imgs)), vm)
    }

    #[pyfunction]
    fn overlay(
        imgs: Vec<ImageRef>,
        x_offset: PyObjectRef,
        y_offset: PyObjectRef,
        xp: PyObjectRef,
        yp: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let offset = point(&x_offset, &y_offset, ("x_offset", "y_offset"), vm)?;
        let (xp, yp) = (x_place(&xp, vm)?, y_place(&yp, vm)?);
        image(Image::overlay(xp, yp, offset, images(imgs)), vm)
    }

    #[pyfunction]
    fn underlay(
        imgs: Vec<ImageRef>,
        x_offset: PyObjectRef,
        y_offset: PyObjectRef,
        xp: PyObjectRef,
        yp: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let offset = point(&x_offset, &y_offset, ("x_offset", "y_offset"), vm)?;
        let (xp, yp) = (x_place(&xp, vm)?, y_place(&yp, vm)?);
        image(Image::underlay(xp, yp, offset, images(imgs)), vm)
    }

    #[pyfunction]
    fn overlay_xy(
        top: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        bottom: ImageRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let at = point(&x, &y, ("x", "y"), vm)?;
        image(top.0.overlay_xy(at, &bottom.0), vm)
    }

    #[pyfunction]
    fn underlay_xy(
        bottom: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        top: ImageRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let at = point(&x, &y, ("x", "y"), vm)?;
        image(bottom.0.underlay_xy(at, &top.0), vm)
    }

    // Scenes

    #[pyfunction]
    fn empty_scene(
        w: PyObjectRef,
        h: PyObjectRef,
        c: ColorRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (w, h) = (length("w", &w, vm)?, length("h", &h, vm)?);
        Ok(PyImage(Image::empty_scene(w, h, c.0)))
    }

    #[pyfunction]
    fn place_image(
        scene: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        img: ImageRef,
        xp: PyObjectRef,
        yp: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (x, y) = (finite("x", &x, vm)?, finite("y", &y, vm)?);
        let (xp, yp) = (x_place(&xp, vm)?, y_place(&yp, vm)?);
        image(scene.0.place_image(x, y, xp, yp, &img.0), vm)
    }

    #[pyfunction]
    fn place_images(
        scene: ImageRef,
        positions: Vec<PyObjectRef>,
        imgs: Vec<ImageRef>,
        xp: PyObjectRef,
        yp: PyObjectRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (xp, yp) = (x_place(&xp, vm)?, y_place(&yp, vm)?);
        // As zip does, the shorter list sets how many images go.
        let n = positions.len().min(imgs.len());
        let at = points(positions.into_iter().take(n).collect(), vm)?;
        image(
            scene
                .0
                .place_images(xp, yp, at.into_iter().zip(images(imgs))),
            vm,
        )
    }

    #[pyfunction]
    fn put_image(
        scene: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        img: ImageRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (x, y) = (finite("x", &x, vm)?, finite("y", &y, vm)?);
        image(scene.0.put_image(x, y, &img.0), vm)
    }

    #[pyfunction]
    fn place_line(
        scene: ImageRef,
        x1: PyObjectRef,
        y1: PyObjectRef,
        x2: PyObjectRef,
        y2: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let from = point(&x1, &y1, ("x1", "y1"), vm)?;
        let to = point(&x2, &y2, ("x2", "y2"), vm)?;
        image(scene.0.place_line(from, to, s.0.clone()), vm)
    }

    #[pyfunction]
    fn place_polygon(
        scene: ImageRef,
        pts: Vec<PyObjectRef>,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let pts = points(pts, vm)?;
        image(scene.0.place_polygon(&pts, s.0.clone()), vm)
    }

    #[pyfunction]
    fn place_curve(
        scene: ImageRef,
        from: PyObjectRef,
        to: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (from, to) = (curve_end(from, 1, vm)?, curve_end(to, 2, vm)?);
        image(scene.0.place_curve(from, to, s.0.clone()), vm)
    }

    #[pyfunction]
    fn place_wedge(
        scene: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        radius: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let center = point(&x, &y, ("x", "y"), vm)?;
        let radius = length("radius", &radius, vm)?;
        let angle = wedge_angle(&angle, vm)?;
        image(scene.0.place_wedge(center, radius, angle, s.0.clone()), vm)
    }

    #[pyfunction]
    fn add_line(
        img: ImageRef,
        x1: PyObjectRef,
        y1: PyObjectRef,
        x2: PyObjectRef,
        y2: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let from = point(&x1, &y1, ("x1", "y1"), vm)?;
        let to = point(&x2, &y2, ("x2", "y2"), vm)?;
        image(img.0.add_line(from, to, s.0.clone()), vm)
    }

    #[pyfunction]
    fn add_polygon(
        img: ImageRef,
        pts: Vec<PyObjectRef>,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let pts = points(pts, vm)?;
        image(img.0.add_polygon(&pts, s.0.clone()), vm)
    }

    #[pyfunction]
    fn add_curve(
        img: ImageRef,
        from: PyObjectRef,
        to: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (from, to) = (curve_end(from, 1, vm)?, curve_end(to, 2, vm)?);
        image(img.0.add_curve(from, to, s.0.clone()), vm)
    }

    #[pyfunction]
    fn add_solid_curve(
        img: ImageRef,
        from: PyObjectRef,
        to: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let (from, to) = (curve_end(from, 1, vm)?, curve_end(to, 2, vm)?);
        image(img.0.add_solid_curve(from, to, s.0.clone()), vm)
    }

    #[pyfunction]
    fn add_wedge(
        img: ImageRef,
        x: PyObjectRef,
        y: PyObjectRef,
        radius: PyObjectRef,
        angle: PyObjectRef,
        s: StyleRef,
        vm: &VirtualMachine,
    ) -> PyResult<PyImage> {
        let center = point(&x, &y, ("x", "y"), vm)?;
        let radius = length("radius", &radius, vm)?;
        let angle = wedge_angle(&angle, vm)?;
        image(img.0.add_wedge(center, radius, angle, s.0.clone()), vm)
    }

    // Svg

    /// The number of the next svg. The page puts every svg of a run into one
    /// HTML document, so two svgs must not share a number.
    static NEXT_SVG: AtomicUsize = AtomicUsize::new(0);

    #[pyfunction]
    fn to_svg(img: ImageRef, vm: &VirtualMachine) -> PyResult<String> {
        let number = NEXT_SVG.fetch_add(1, Ordering::Relaxed);
        img.0.to_svg(number).map_err(|e| too_large(e, vm))
    }

    #[pyfunction]
    fn to_repl_svg(img: ImageRef, vm: &VirtualMachine) -> PyResult<String> {
        let number = NEXT_SVG.fetch_add(1, Ordering::Relaxed);
        img.0.to_repl_svg(number).map_err(|e| too_large(e, vm))
    }
}
