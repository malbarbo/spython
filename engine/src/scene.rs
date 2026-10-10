//! The `_spython_scene` module: the [`Scene`] of an `Image` of
//! `spython.image`, the svg that sinteract writes of it, the images that a
//! bitmap keeps, and the box of a path. Every target builds the scene the
//! same way, so an image looks the same on the page and in a window.

pub(crate) use _spython_scene::module_def;
// Only a display draws a scene of its own, and the page has none.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use _spython_scene::draw;

#[rustpython_vm::pymodule(name = "_spython_scene")]
pub(crate) mod _spython_scene {
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use kurbo::{BezPath, ParamCurveExtrema, Rect, SvgArc, Vec2};
    use rustpython_vm::builtins::PyStrRef;
    use rustpython_vm::function::ArgIntoFloat;
    use rustpython_vm::{
        PyObject, PyObjectRef, PyPayload, PyResult, TryFromObject, VirtualMachine, pyclass,
    };
    use sinteract::asset::{ImageError, decode_data_uri};
    use sinteract::renderer::Renderer;
    use sinteract::renderer::svg::SvgRenderer;
    use sinteract::scene::{
        Bitmap, ClipPath, Dash, FillRule, FontStyle, Image, LineCap, LineJoin, Paint, Path,
        PathBuilder, PathStyle, Sampling, Scene, Text, TextSpec,
    };

    /// The image in `data_uri`, with the width and the height of the
    /// original. A `_Bitmap` keeps the image, so a frame draws it with no
    /// decode. An image larger than the limit of sinteract shrinks, and
    /// shows at the size of the original.
    #[pyfunction]
    fn load_image(data_uri: PyStrRef, vm: &VirtualMachine) -> PyResult<(PyLoadedImage, f64, f64)> {
        let blob = data_uri
            .to_str()
            .and_then(decode_data_uri)
            .ok_or_else(|| vm.new_value_error("a bitmap needs a data URI in base64"))?;
        loaded(blob).map_err(|e| vm.new_value_error(e.to_string()))
    }

    /// The image in the file at `path`, as [`load_image`] gives it.
    #[pyfunction]
    fn load_bitmap(path: String, vm: &VirtualMachine) -> PyResult<(PyLoadedImage, f64, f64)> {
        let blob = crate::host::load_bitmap(&path).map_err(|e| vm.new_runtime_error(e))?;
        loaded(blob).map_err(|e| vm.new_runtime_error(format!("{path}: {e}")))
    }

    /// The image in `blob`, with the width and the height of the original.
    fn loaded(blob: Vec<u8>) -> Result<(PyLoadedImage, f64, f64), ImageError> {
        let (image, head) = Image::load_with_head(blob)?;
        let size = (f64::from(head.width), f64::from(head.height));
        Ok((PyLoadedImage { image }, size.0, size.1))
    }

    /// The number in the id prefix of the next svg. The page puts every svg
    /// of a run into one HTML document, so two svgs must not share an id.
    static NEXT_SVG: AtomicUsize = AtomicUsize::new(0);

    /// The svg of `image`, an `Image` of `spython.image`, on a scene of
    /// `width` by `height`.
    #[pyfunction]
    fn to_svg(
        image: PyObjectRef,
        width: ArgIntoFloat,
        height: ArgIntoFloat,
        vm: &VirtualMachine,
    ) -> PyResult<String> {
        let mut scene = Scene::new(f64::from(width) as f32, f64::from(height) as f32);
        draw(&image, &mut scene, vm)?;
        let prefix = format!("s{}-", NEXT_SVG.fetch_add(1, Ordering::Relaxed));
        let mut renderer = SvgRenderer::with_id_prefix(&prefix).expect("a valid id prefix");
        let Ok(_) = renderer.render(&scene);
        Ok(renderer.into_string())
    }

    /// The box around the path of `commands` as the least x, the least y,
    /// the greatest x and the greatest y. An arc counts as the cubics that
    /// sinteract draws for it, so the box fits the drawing.
    #[pyfunction]
    fn path_box(commands: PyObjectRef, vm: &VirtualMachine) -> PyResult<(f64, f64, f64, f64)> {
        let mut path = BezPath::new();
        for cmd in vm.extract_elements_with(&commands, Ok)? {
            let at = |name: &'static str| -> PyResult<kurbo::Point> {
                let p = cmd.get_attr(name, vm)?;
                Ok(kurbo::Point::new(float(&p, "x", vm)?, float(&p, "y", vm)?))
            };
            let class = cmd.class().name().to_owned();
            let end = at(match class.as_str() {
                "_MoveTo" | "_LineTo" => "p",
                _ => "end",
            })?;
            // A path that starts with a segment other than a move starts at
            // the end of that segment.
            let Some(from) = path.elements().last().and_then(|el| el.end_point()) else {
                path.move_to(end);
                continue;
            };
            match class.as_str() {
                "_MoveTo" => path.move_to(end),
                "_LineTo" => path.line_to(end),
                "_QuadTo" => path.quad_to(at("control")?, end),
                "_CubicTo" => path.curve_to(at("c1")?, at("c2")?, end),
                "_ArcTo" => {
                    let arc = SvgArc {
                        from,
                        to: end,
                        radii: Vec2::new(float(&cmd, "rx", vm)?, float(&cmd, "ry", vm)?),
                        x_rotation: float(&cmd, "rotation", vm)?.to_radians(),
                        large_arc: cmd.get_attr("large_arc", vm)?.try_to_bool(vm)?,
                        sweep: cmd.get_attr("sweep", vm)?.try_to_bool(vm)?,
                    };
                    match kurbo::Arc::from_svg_arc(&arc) {
                        Some(arc) => {
                            let tolerance =
                                sinteract::scene::arc_tolerance(arc.radii.x.max(arc.radii.y));
                            path.extend(arc.append_iter(tolerance));
                        }
                        None => path.line_to(end),
                    }
                }
                _ => return Err(vm.new_type_error(format!("cannot measure a {class}"))),
            }
        }
        // A move adds its point even when no segment leaves it.
        let mut points = path.elements().iter().filter_map(|el| el.end_point());
        let Some(first) = points.next() else {
            return Ok((0.0, 0.0, 0.0, 0.0));
        };
        let rect = points.fold(Rect::from_points(first, first), |rect, p| rect.union_pt(p));
        let rect = path
            .segments()
            .fold(rect, |rect, seg| rect.union(seg.bounding_box()));
        Ok((rect.x0, rect.y0, rect.x1, rect.y1))
    }

    /// The image that [`load_image`] or [`load_bitmap`] returns. Python only
    /// passes it along.
    #[pyattr]
    #[pyclass(name = "LoadedImage")]
    #[derive(Debug, PyPayload)]
    pub struct PyLoadedImage {
        image: Image,
    }

    #[pyclass]
    impl PyLoadedImage {}

    /// Put `image` into `scene`. A crop inside another becomes one clip of
    /// the two boxes together, so no clip of the scene holds another, and
    /// the nesting limit of sinteract drops nothing.
    pub(crate) fn draw(image: &PyObject, scene: &mut Scene, vm: &VirtualMachine) -> PyResult<()> {
        let mut runs = Vec::new();
        walk(image, &mut runs, vm)?;
        for (region, leaves) in runs {
            let Some(region) = region else {
                leaves.into_iter().for_each(|leaf| leaf.add_to(scene));
                continue;
            };
            let Some((&(x, y), rest)) = region.split_first() else {
                continue;
            };
            let mut clip = ClipPath::builder(FillRule::default(), x as f32, y as f32);
            for &(x, y) in rest {
                clip = clip.line_to(x as f32, y as f32);
            }
            scene.clip(clip.build(), |inner| {
                leaves.into_iter().for_each(|leaf| leaf.add_to(inner));
            });
        }
        Ok(())
    }

    /// The area that the crops around an element leave visible, as the
    /// corners of a convex polygon. `None` is no crop, and no corners is an
    /// area with nothing in it.
    type Region = Option<Vec<(f64, f64)>>;

    /// An element of the scene, before it goes into a clip.
    enum Leaf {
        Path(Path),
        Text(Text),
        Bitmap(Bitmap),
    }

    impl Leaf {
        fn add_to(self, scene: &mut Scene) {
            match self {
                Leaf::Path(path) => scene.add_path(path),
                Leaf::Text(text) => scene.add_text(text),
                Leaf::Bitmap(bitmap) => scene.add_bitmap(bitmap),
            }
        }
    }

    /// Append the elements of `image` to `runs`, which groups the elements
    /// in a row that share a region, in drawing order. The walk keeps the
    /// nodes still to visit on a stack of its own, so an image of any depth
    /// fits in the native stack.
    fn walk(
        image: &PyObject,
        runs: &mut Vec<(Region, Vec<Leaf>)>,
        vm: &VirtualMachine,
    ) -> PyResult<()> {
        let mut pending = vec![(image.to_owned(), Rc::new(None))];
        while let Some((image, region)) = pending.pop() {
            let class = image.class().name().to_owned();
            let leaf = match class.as_str() {
                "_Path" => path(&image, vm)?.map(Leaf::Path),
                "_Line" => Some(Leaf::Path(line(&image, vm)?)),
                "_Combination" => {
                    // The stack gives back the last node first, and `a`
                    // draws before `b`.
                    pending.push((image.get_attr("b", vm)?, Rc::clone(&region)));
                    pending.push((image.get_attr("a", vm)?, Rc::clone(&region)));
                    None
                }
                "_Crop" => {
                    let corners = corners(affine(&*image.get_attr("box", vm)?, vm)?);
                    let inside = Rc::new(intersect(&region, &corners));
                    pending.push((image.get_attr("image", vm)?, inside));
                    None
                }
                "_Text" => text(&image, vm)?.map(Leaf::Text),
                "_Bitmap" => {
                    let transform = affine(&*image.get_attr("box", vm)?, vm)?;
                    let loaded = image.get_attr("loaded", vm)?;
                    let loaded = loaded.downcast_ref::<PyLoadedImage>().ok_or_else(|| {
                        vm.new_type_error("a bitmap needs the image of load_image")
                    })?;
                    Some(Leaf::Bitmap(Bitmap {
                        image: loaded.image.clone(),
                        transform,
                        sampling: Sampling::Smooth,
                    }))
                }
                _ => return Err(vm.new_type_error(format!("cannot draw a {class}"))),
            };
            let Some(leaf) = leaf else {
                continue;
            };
            if region.as_ref().as_ref().is_some_and(Vec::is_empty) {
                continue;
            }
            match runs.last_mut() {
                Some((last, leaves)) if last == region.as_ref() => leaves.push(leaf),
                _ => runs.push((region.as_ref().clone(), vec![leaf])),
            }
        }
        Ok(())
    }

    /// The corners of the box of `m`, in the convention of
    /// [`ClipPath::from_transform`].
    fn corners(m: [f32; 6]) -> [(f64, f64); 4] {
        let [a, b, c, d, e, f] = m.map(f64::from);
        [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
            .map(|(u, v)| (a * u + c * v + e, b * u + d * v + f))
    }

    /// How far a point may be outside a box and still count as inside, so
    /// a box that matches the region up to rounding does not cut it.
    const INSIDE_TOLERANCE: f64 = 1.0e-3;

    /// The part of `region` inside the box with `corners`. A box with no
    /// area, or one that is not finite, leaves nothing.
    fn intersect(region: &Region, corners: &[(f64, f64); 4]) -> Region {
        let finite = corners.iter().all(|&(x, y)| x.is_finite() && y.is_finite());
        let area = cross(corners[0], corners[1], corners[2]);
        if !finite || area == 0.0 {
            return Some(Vec::new());
        }
        // The sign makes the distance to an edge positive inside the box,
        // whichever way its corners turn.
        let sign = area.signum();
        let edges = || {
            corners
                .iter()
                .copied()
                .zip(corners.iter().copied().cycle().skip(1))
        };
        let distance = |(p, q): ((f64, f64), (f64, f64)), r: (f64, f64)| {
            sign * cross(p, q, r) / (q.0 - p.0).hypot(q.1 - p.1)
        };
        let Some(region) = region else {
            return Some(corners.to_vec());
        };
        let contains = |r: (f64, f64)| edges().all(|e| distance(e, r) >= -INSIDE_TOLERANCE);
        if region.iter().all(|&r| contains(r)) {
            return Some(region.clone());
        }
        // Sutherland-Hodgman. The box is convex, so each of its edges cuts
        // away what lies outside it.
        let mut out = region.clone();
        for edge in edges() {
            let points = std::mem::take(&mut out);
            let sides = points
                .iter()
                .copied()
                .zip(points.iter().copied().cycle().skip(1));
            for (p, q) in sides {
                let (dp, dq) = (distance(edge, p), distance(edge, q));
                if dp >= 0.0 {
                    out.push(p);
                }
                if (dp >= 0.0) != (dq >= 0.0) {
                    let t = dp / (dp - dq);
                    out.push((p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1)));
                }
            }
            // A corner on the edge comes back as itself and as the point
            // where the edge cuts the side.
            let same =
                |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1) < INSIDE_TOLERANCE;
            out.dedup_by(|p, q| same(*p, *q));
            while let [first, .., last] = out[..]
                && same(first, last)
            {
                out.pop();
            }
        }
        if out.len() < 3 {
            out.clear();
        }
        Some(out)
    }

    /// Twice the signed area of the triangle `p`, `q`, `r`.
    fn cross(p: (f64, f64), q: (f64, f64), r: (f64, f64)) -> f64 {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
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
        let point = pixel_point(&style, vm);
        let (x, y) = point(first.get_attr("p", vm)?)?;
        let mut path_style = style.path_style();
        path_style.closed = image.get_attr("closed", vm)?.try_to_bool(vm)?;
        let mut builder = Path::builder(path_style, x, y);
        for cmd in rest {
            builder = segment(builder, cmd, &point, vm)?;
        }
        Ok(Some(builder.build()))
    }

    /// The path of a `_Line`, from one end to the other.
    fn line(image: &PyObject, vm: &VirtualMachine) -> PyResult<Path> {
        let style = Style::read(&*image.get_attr("style", vm)?, vm)?;
        let point = pixel_point(&style, vm);
        let (x, y) = point(image.get_attr("start", vm)?)?;
        let (end_x, end_y) = point(image.get_attr("end", vm)?)?;
        Ok(Path::builder(style.path_style(), x, y)
            .line_to(end_x, end_y)
            .build())
    }

    /// Reads a point for a path with `style`. A thin outline on a half pixel
    /// covers one row of pixels, and on a whole pixel it blurs over two, so
    /// an aligned style puts the point on the center of its pixel.
    fn pixel_point<'a>(
        style: &'a Style,
        vm: &'a VirtualMachine,
    ) -> impl Fn(PyObjectRef) -> PyResult<(f32, f32)> + 'a {
        let snap = |v: f64| {
            if style.aligned() {
                (v + FP_EPSILON).floor() + 0.5
            } else {
                v
            }
        };
        move |p: PyObjectRef| {
            Ok((
                snap(float(&p, "x", vm)?) as f32,
                snap(float(&p, "y", vm)?) as f32,
            ))
        }
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

    /// The width and the height of the box around `text` in `font`. sinteract
    /// measures with the fonts that it draws with, so a text has the same
    /// size on every target. A size that cannot draw measures nothing.
    #[pyfunction]
    fn text_metrics(text: String, font: PyObjectRef, vm: &VirtualMachine) -> PyResult<(f64, f64)> {
        let spec = text_spec(&font, text, vm)?;
        let metrics =
            sinteract::text::measure(&spec.family, spec.weight, spec.style, spec.size, &spec.text);
        Ok(metrics.map_or((0.0, 0.0), |m| {
            (f64::from(m.width()), f64::from(m.height()))
        }))
    }

    /// `text` in `font`, a `Font` of `spython.font`. The measure and the
    /// drawing of a text both read the font here, so the box they get is the
    /// same.
    fn text_spec(font: &PyObject, text: String, vm: &VirtualMachine) -> PyResult<TextSpec> {
        Ok(TextSpec {
            size: float(font, "size", vm)? as f32,
            family: string(font, "family", vm)?.into(),
            weight: match enum_value(font, "weight", vm)?.as_str() {
                "lighter" => 300,
                "bold" => 700,
                _ => 400,
            },
            style: match enum_value(font, "style", vm)?.as_str() {
                "italic" => FontStyle::Italic,
                "oblique" => FontStyle::Oblique,
                _ => FontStyle::Normal,
            },
            text,
        })
    }

    /// The text of a `_Text`, or `None` when it draws nothing.
    fn text(image: &PyObject, vm: &VirtualMachine) -> PyResult<Option<Text>> {
        let style = Style::read(&*image.get_attr("style", vm)?, vm)?;
        let font = image.get_attr("font", vm)?;
        let transform = affine(&*image.get_attr("box", vm)?, vm)?;
        let spec = text_spec(&font, string(image, "text", vm)?, vm)?;
        let Some(mut text) = spec.fit_transform(transform) else {
            return Ok(None);
        };
        let path_style = style.path_style();
        text.fill = path_style.fill;
        text.stroke = path_style.stroke;
        text.stroke_width = path_style.stroke_width;
        text.underline = font.get_attr("underline", vm)?.try_to_bool(vm)?;
        Ok(Some(text))
    }

    /// The affine of a `_Box`, which takes the unit square centred on the
    /// origin to the box.
    fn affine(b: &PyObject, vm: &VirtualMachine) -> PyResult<[f32; 6]> {
        let point = |name: &'static str| -> PyResult<(f32, f32)> {
            let p = b.get_attr(name, vm)?;
            Ok((float(&p, "x", vm)? as f32, float(&p, "y", vm)? as f32))
        };
        let (cx, cy) = point("center")?;
        let (ux, uy) = point("x_axis")?;
        let (vx, vy) = point("y_axis")?;
        Ok([ux, uy, vx, vy, cx, cy])
    }

    /// A `Style` of `spython.style`, with a default for each attribute left
    /// out.
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
        /// the first wins.
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
        /// `false` otherwise. A width of 1 is the width an outline has by
        /// default, so it draws the same.
        fn aligned(&self) -> bool {
            self.stroke.is_some()
                && self.fill.is_none()
                && self.stroke_width.is_none_or(|w| w == 1.0)
        }

        fn path_style(&self) -> PathStyle {
            let outline = self.stroke.is_some();
            // A shape with no fill is black, as in SVG, and an outline is not
            // filled.
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
                // An outline with no join of its own gets a limit of 10.
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

    /// The number at `obj.name`, or 0 when it is within [`FP_EPSILON`] of
    /// 0. A sine or a cosine leaves a residue near 0 that differs from one
    /// libm to another, and the svg would write the residue in full.
    fn float(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<f64> {
        let v: f64 = ArgIntoFloat::try_from_object(vm, obj.get_attr(name, vm)?)?.into();
        Ok(if v.abs() < FP_EPSILON { 0.0 } else { v })
    }

    fn string(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<String> {
        String::try_from_object(vm, obj.get_attr(name, vm)?)
    }

    /// The `value` of the `Enum` at `obj.name`.
    fn enum_value(obj: &PyObject, name: &'static str, vm: &VirtualMachine) -> PyResult<String> {
        string(&*obj.get_attr(name, vm)?, "value", vm)
    }
}
