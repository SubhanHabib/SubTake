//! The timeline: tracks, regions, the scrubber and its drag gestures.

use super::*;
use subtake_ui::layered;

/// How far the console's zoom out and in step, and the most the timeline
/// magnifies: a hundredth of the take across the lanes.
const TIMELINE_ZOOM_STEP: f32 = 1.5;
const TIMELINE_ZOOM_MAX: f32 = 100.;

/// How far the gallery's held region has been moved or trimmed, in seconds.
const GALLERY_HOLD_SECONDS: f32 = 4.;

/// The ruler's tick intervals, in seconds. It takes the smallest that keeps
/// its labels `RULER_LABEL_SPACING` apart at the current zoom.
const RULER_INTERVALS: [f32; 9] = [1., 2., 5., 10., 15., 30., 60., 120., 300.];

/// A time as the ruler and the playhead chip write it: `m:ss`, or `m:ss.cc`
/// with hundredths — the transport's own format, so the two agree.
fn ruler_clock(seconds: f32, hundredths: bool) -> String {
    let centis = (seconds.max(0.) * 100.).round() as u64;
    let (minutes, rest) = (centis / 6000, centis % 6000);
    if hundredths {
        format!("{minutes}:{:02}.{:02}", rest / 100, rest % 100)
    } else {
        format!("{minutes}:{:02}", rest / 100)
    }
}

/// The icon on a region's plate: what kind of region it is. A native
/// marker has none.
fn region_icon(region: &Region) -> Option<&'static str> {
    match region.kind.as_str() {
        "zoomRegions" => Some("MagnifyingGlassPlus-regular"),
        "speedRegions" => Some("Timer-regular"),
        "trimRegions" => Some("Scissors-regular"),
        "annotationRegions" if region.arrow => Some("ArrowUpRight-regular"),
        "annotationRegions" => Some("TextT-regular"),
        "autoCaptions" => Some("ClosedCaptioning-regular"),
        "audioRegions" | Region::TAKE_AUDIO => Some("MusicNotes-regular"),
        "clipRegions" | Region::TAKE_CLIP => Some("FilmStrip-regular"),
        _ => None,
    }
}

/// The thumbnail strip cut into its frames, so a clip's tile can fit one
/// frame and round its own corners, which a crop of the whole strip cannot.
fn split_frames(strip: &RenderImage) -> Vec<Arc<RenderImage>> {
    let size = strip.size(0);
    let (width, height) = (size.width.0 as u32, size.height.0 as u32);
    let Some(strip) = strip
        .as_bytes(0)
        .and_then(|bytes| image::RgbaImage::from_raw(width, height, bytes.to_vec()))
    else {
        return Vec::new();
    };
    let frames = crate::media::TIMELINE_FRAMES;
    let frame = width / frames;
    (0..frames)
        .map(|i| {
            let pixels = image::imageops::crop_imm(&strip, i * frame, 0, frame, height).to_image();
            Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                pixels
            )]))
        })
        .collect()
}

/// A clip: the recording's frames, one tile per 88 across, the frame each
/// shows the one nearest the time at its middle. The first and last tiles
/// round the clip's ends; a last tile too short to take the curve is folded
/// into the one before it. Only the tiles in sight are built: over the
/// track, under the headers to their centres and out to the console's
/// edge. `cut` is how much of the clip is tucked away under the headers;
/// the tile it falls in is cropped there and takes the round end. A tile
/// or divider that starts within the round end's curve is rounded or
/// shortened to stay inside it, since gpui clips only to a rectangle.
fn clip_tiles(
    frames: &[Arc<RenderImage>],
    (start, end): (f32, f32),
    (left, width, height, cut): (f32, f32, f32, f32),
    (track_width, duration): (f32, f32),
) -> Vec<AnyElement> {
    let pitch = Theme::CLIP_TILE_WIDTH + Theme::CLIP_TILE_DIVIDER;
    let radius = height / 2.;
    let seen_right = track_width + Theme::PANEL_PADDING;
    let first = (cut / pitch).floor() as usize;
    let last = ((seen_right - left).min(width) / pitch).ceil() as usize;
    let mut tiles = Vec::new();
    for tile in first..=last {
        let x = tile as f32 * pitch;
        if x >= width || frames.is_empty() {
            break;
        }
        // What is left of a clip narrower than it is tall is one frame,
        // cut to the circle its lane is left as.
        let rest = width - x - Theme::CLIP_TILE_WIDTH;
        let closing =
            (cut > 0. && width - cut < height) || rest < Theme::CLIP_TILE_DIVIDER + radius;
        let w = if closing {
            width - x
        } else {
            Theme::CLIP_TILE_WIDTH
        };
        let time = start + (x + w / 2.) / width * (end - start);
        let index = ((time / duration.max(f32::EPSILON) * frames.len() as f32).floor() as usize)
            .min(frames.len() - 1);
        if x < cut {
            if x + w > cut {
                tiles.push(
                    cut_tile(frames[index].clone(), cut - x, (radius, closing))
                        .absolute()
                        .left(px(x))
                        .top_0()
                        .w(px(w))
                        .h(px(height))
                        .into_any_element(),
                );
            }
        } else {
            // Rounded to the round end's curve where it starts inside it:
            // an arc `radius - x` across at its left runs on the curve.
            let mut frame = img(frames[index].clone())
                .absolute()
                .left(px(x))
                .top_0()
                .w(px(w))
                .h(px(height))
                .object_fit(ObjectFit::Cover)
                .rounded_l(px((radius - (x - cut)).max(0.)));
            if closing {
                frame = frame.rounded_r(px(radius));
            }
            tiles.push(frame.into_any_element());
        }
        if closing {
            break;
        }
        // A divider inside the round end's curve is as tall as the curve is
        // there.
        let into = x + Theme::CLIP_TILE_WIDTH - cut;
        let tall = if into < radius {
            2. * (radius * radius - (radius - into).max(0.).powi(2)).sqrt()
        } else {
            height
        };
        tiles.push(
            div()
                .absolute()
                .left(px(x + Theme::CLIP_TILE_WIDTH))
                .top(px((height - tall) / 2.))
                .w(px(Theme::CLIP_TILE_DIVIDER))
                .h(px(tall))
                .bg(gpui::black().opacity(Theme::CLIP_DIVIDER_ALPHA))
                .into_any_element(),
        );
    }
    tiles
}

/// A tile whose left `cut` is tucked under the headers: its frame cropped
/// there rather than refitted to what is left, so it does not slide as the
/// timeline scrolls, and rounded at the cut as the clip's own end is.
/// `closing` rounds its right end too. Left narrower than the lane is
/// tall, it is drawn that wide under the tiles after it, its frame slid
/// along to fill it, since gpui rounds a corner no wider than half the
/// quad; what is left of it is under a header. A closing tile left
/// narrower than that is a circle as wide, centred, as [`tucked`] leaves
/// its lane.
fn cut_tile(frame: Arc<RenderImage>, cut: f32, (radius, closing): (f32, bool)) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut fitted = ObjectFit::Cover.get_bounds(bounds, frame.size(0));
            let left = bounds.left() + px(cut);
            let right = if closing {
                bounds.right()
            } else {
                bounds.right().max(left + px(2. * radius))
            };
            if right > fitted.right() {
                fitted.origin.x += right - fitted.right();
            }
            let inset = if closing {
                ((bounds.size.height - (right - left)) / 2.).max(px(0.))
            } else {
                px(0.)
            };
            let shown = Bounds::from_corners(
                point(left, bounds.top() + inset),
                point(right, bounds.bottom() - inset),
            )
            .intersect(&fitted);
            let end = if closing { px(radius) } else { px(0.) };
            let radii = Corners {
                top_left: px(radius),
                bottom_left: px(radius),
                top_right: end,
                bottom_right: end,
            }
            .clamp_radii_for_quad_size(shown.size);
            window
                .paint_image_fitted(shown, fitted, radii, frame.clone(), 0, false)
                .ok();
        },
    )
}

/// A clip's name on a `card` chip over its frames, with the `FilmStrip`
/// plate. A clip too short to leave its name four letters shows the plate
/// alone. Not drawn by the design: the `saturate()` beside the chip's blur,
/// which gpui has no filter for; the blur itself, where the window has no
/// glass; and a clip shorter than the chip itself, which shows its frames
/// with no chip.
fn clip_chip(label: &str, width: f32, theme: Theme) -> Option<impl IntoElement> {
    let inset = Theme::CLIP_CHIP_INSET;
    if width < Theme::CLIP_CHIP_HEIGHT + 2. * inset {
        return None;
    }
    let room = width
        - 2. * inset
        - Theme::REGION_PLATE_INSET
        - Theme::CLIP_CHIP_PLATE
        - Theme::CLIP_CHIP_GAP
        - Theme::REGION_PADDING_END;
    let labelled = room >= Theme::REGION_LABEL_MIN_ROOM;
    // Blurring the frames under it, in a layer of its own: the console is
    // one layer, and in one layer the frames paint over the chip's fill.
    Some(layered(frosted(
        Theme::CLIP_CHIP_HEIGHT / 2.,
        Theme::CLIP_CHIP_BLUR,
        div()
            .absolute()
            .left(px(inset))
            .top(px(inset))
            .h(px(Theme::CLIP_CHIP_HEIGHT))
            .max_w(px(width - 2. * inset))
            .flex()
            .items_center()
            .gap(px(Theme::CLIP_CHIP_GAP))
            .pl(px(Theme::REGION_PLATE_INSET))
            .when(labelled, |el| el.pr(px(Theme::REGION_PADDING_END)))
            .when(!labelled, |el| el.pr(px(Theme::REGION_PLATE_INSET)))
            .rounded_full()
            .bg(theme.card)
            // A border rather than an inset hairline, which gpui paints
            // under the chip's own fill.
            .border(px(Theme::HAIRLINE_WIDTH))
            .border_color(theme.line)
            .text_size(px(Theme::FONT_BODY))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text)
            .child(
                div()
                    .flex_none()
                    .size(px(Theme::CLIP_CHIP_PLATE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme.plate)
                    .child(subtake_ui::icon_sized(
                        "FilmStrip-regular",
                        Theme::CLIP_CHIP_ICON,
                        theme.text,
                    )),
            )
            .when(labelled, |el| {
                el.child(
                    div()
                        .min_w_0()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(label.to_string()),
                )
            }),
    )))
}

/// The waveform image read back into its peaks: for each column, how much
/// of its height is drawn, 0 to 1. ffmpeg draws the sound mirrored about the
/// middle, so the drawn span is the peak.
fn read_peaks(image: &RenderImage) -> Arc<[f32]> {
    let size = image.size(0);
    let (width, height) = (size.width.0 as usize, size.height.0 as usize);
    let Some(bytes) = image.as_bytes(0) else {
        return Arc::from([]);
    };
    (0..width)
        .map(|x| {
            let drawn = (0..height)
                .filter(|y| bytes[(y * width + x) * 4 + 3] > 0)
                .count();
            drawn as f32 / height.max(1) as f32
        })
        .collect()
}

/// The recording's sound over its region: bars on a 4.5 pitch across the
/// row, each as tall as the loudest peak in its span, in the region's ink,
/// and at 35% past the playhead. `track` is the track column, to read each
/// bar's time from where it is drawn.
fn waveform(
    peaks: Arc<[f32]>,
    track: Rc<Cell<Bounds<Pixels>>>,
    (offset, visible, duration, playhead): (f32, f32, f32, f32),
    ink: Hsla,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let track = track.get();
            let track_width = f32::from(track.size.width).max(f32::EPSILON);
            let time = |x: f32| offset + (x - f32::from(track.left())) / track_width * visible;
            let index = |t: f32| {
                ((t / duration.max(f32::EPSILON) * peaks.len() as f32).max(0.) as usize)
                    .min(peaks.len().saturating_sub(1))
            };
            let row = f32::from(bounds.size.height);
            // From the first bar in sight to the last, so a zoomed take
            // does not paint thousands of bars out of sight. The lanes are
            // seen past the track column: under the headers to their
            // centres, and out to the console's edge.
            let seen_left = f32::from(track.left()) + HEADER_CENTRE;
            let seen_right = f32::from(track.right()) + Theme::PANEL_PADDING;
            let left = f32::from(bounds.left());
            let skipped = ((seen_left - left) / Theme::WAVEFORM_PITCH).floor().max(0.);
            let mut x = left + skipped * Theme::WAVEFORM_PITCH;
            let right = f32::from(bounds.right());
            while !peaks.is_empty() && x + Theme::WAVEFORM_BAR <= right && x < seen_right {
                let (from, to) = (index(time(x)), index(time(x + Theme::WAVEFORM_PITCH)));
                let peak = peaks[from..=to.max(from)]
                    .iter()
                    .copied()
                    .fold(0., f32::max);
                let bar = row
                    * (Theme::WAVEFORM_FLOOR
                        + (Theme::WAVEFORM_CEILING - Theme::WAVEFORM_FLOOR) * peak);
                let past = time(x + Theme::WAVEFORM_BAR / 2.) <= playhead;
                let color = if past {
                    ink
                } else {
                    ink.opacity(Theme::WAVEFORM_AHEAD_ALPHA)
                };
                window.paint_quad(
                    fill(
                        Bounds::new(
                            point(px(x), bounds.top() + px((row - bar) / 2.)),
                            size(px(Theme::WAVEFORM_BAR), px(bar)),
                        ),
                        color,
                    )
                    .corner_radii(px(Theme::WAVEFORM_BAR_RADIUS)),
                );
                x += Theme::WAVEFORM_PITCH;
            }
        },
    )
    .flex_1()
    .min_w_0()
    .h(px(Theme::WAVEFORM_HEIGHT))
}

/// The centre of a lane's header, from the track column's left. What a lane
/// shows over its fill, its label, plate, sound and chip, is cut there,
/// where the header's circle covers it top to bottom.
const HEADER_CENTRE: f32 = -Theme::LANE_HEADER_GAP - Theme::LANE_HEADER / 2.;

/// Where a lane `height` tall is cut on the left, from the track column's
/// left: half its height before its header's centre, so what runs under the
/// header ends in a round end on the header's circle, as a region's pill
/// does under its plate. A lane taller than its header runs past the
/// circle by the difference.
fn tuck(height: f32) -> f32 {
    HEADER_CENTRE - height / 2.
}

/// A lane's span from `start` to `end`, cut at [`tuck`]: its left edge
/// there when it starts before it, placed `top` down and `height` tall.
/// `None` when it ends before it, out of sight. `cut` is how much of the
/// span is tucked away. Not drawn by the design: what is left of a span
/// narrower than it is tall, where its round end on the header's circle
/// meets its own, is a circle that wide, `inset` in from its top and
/// bottom, since gpui rounds a corner no wider than half the quad and the
/// span would show as a sliver the lane's height past the circle.
fn tucked<E: Styled>(
    el: E,
    (start, end, top, height): (f32, f32, f32, f32),
    (offset, visible, track_width): (f32, f32, f32),
) -> Option<(E, f32, f32)> {
    let edge = tuck(height);
    let (left, right) = (
        track_width * (start - offset) / visible,
        track_width * (end - offset) / visible,
    );
    if track_width > 0. && right <= edge {
        return None;
    }
    let cut = if track_width > 0. {
        (edge - left).max(0.)
    } else {
        0.
    };
    Some(if cut > 0. {
        let inset = ((height - (right - edge)) / 2.).max(0.);
        (
            el.left(px(edge))
                .right(relative(1. - (end - offset) / visible))
                .top(px(top + inset))
                .h(px(height - 2. * inset)),
            cut,
            inset,
        )
    } else {
        (
            el.left(relative((start - offset) / visible))
                .w(relative(((end - start) / visible).max(0.001)))
                .min_w(px(Theme::GAP))
                .top(px(top))
                .h(px(height)),
            0.,
            0.,
        )
    })
}

/// The glyph on a lane's header: what the lane holds.
fn lane_icon(label: &str) -> &'static str {
    match label {
        "Zoom" => "MagnifyingGlassPlus-regular",
        "Clip" => "FilmStrip-regular",
        "Annotation" => "TextT-regular",
        "Caption" => "ClosedCaptioning-regular",
        _ => "MusicNotes-regular",
    }
}

/// A lane's height: the clip lane is taller, for the frames it carries.
fn lane_height(label: &str) -> f32 {
    if label == "Clip" {
        Theme::CLIP_LANE_HEIGHT
    } else {
        Theme::LANE_HEIGHT
    }
}

/// How wide a label is in Geist Mono at `size`, known before layout.
fn mono_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * Theme::MONO_ADVANCE
}

/// The playhead's soft light: `accent_glow`, spread evenly around a part.
fn glow(theme: Theme, blur: f32) -> BoxShadow {
    BoxShadow {
        color: theme.accent_glow,
        offset: point(px(0.), px(0.)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

/// The bubble's tail: a small downward triangle, which no div can be.
fn playhead_tail(color: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let mut path = PathBuilder::fill();
            path.move_to(bounds.origin);
            path.line_to(bounds.top_right());
            path.line_to(point(bounds.center().x, bounds.bottom()));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
}

/// A lane's header: a 44 circle on `sunk` with a hairline and the lane's
/// glyph, centred in a row the lane's own height so the two line up. It
/// turns the lane off and on. Off, it loses its plate, its glyph goes
/// `muted` with a slash through it, and its tooltip says so. Not drawn by
/// the design: the press's 0.94 scale, which gpui cannot give a div; and an
/// off header does not blur a zoomed lane running under it, since the blur
/// on its own reads as the plate it has lost.
fn lane_header(
    label: &str,
    off: bool,
    theme: Theme,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let id: ElementId = SharedString::from(format!("lane-{label}")).into();
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let (rest, hovered) = if off {
        (gpui::transparent_black(), theme.hover)
    } else {
        (theme.sunk, theme.sunk2)
    };
    let ink = if off { theme.muted } else { theme.text };
    let tip = format!("{} · {}", lane_name(label), lane_state(label, off));
    let button = subtake_ui::pressable(
        div()
            .id(id)
            .relative()
            .size(px(Theme::LANE_HEADER))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(subtake_ui::motion::hover_blend(&hover_key, rest, hovered))
            .when(!off, |el| {
                el.shadow(vec![hairline(theme.line, Theme::HAIRLINE_WIDTH)])
            }),
        theme,
        Some(theme.press),
        hover_key.clone(),
    )
    .child(subtake_ui::icon_sized(
        lane_icon(label),
        Theme::LANE_HEADER_ICON,
        ink,
    ))
    .when(off, |el| el.child(lane_slash(ink)))
    .tooltip(move |_, cx| tooltip(tip.clone(), theme, cx))
    .on_click(on_toggle);
    div()
        .h(px(lane_height(label)))
        .flex()
        .flex_none()
        .items_center()
        // On, frosted, so a zoomed lane running on under it blurs away
        // behind its glyph rather than cutting through it.
        .child(if off {
            layered(button).into_any_element()
        } else {
            frosted(Theme::LANE_HEADER / 2., Theme::LANE_HEADER_BLUR, button).into_any_element()
        })
}

/// A lane's name in its header's tooltip.
fn lane_name(label: &str) -> &'static str {
    match label {
        "Zoom" => "Zooms",
        "Clip" => "Clips",
        "Annotation" => "Annotations",
        "Caption" => "Captions",
        _ => "Audio",
    }
}

/// What turning a lane off means: the audio lane is muted, and every other
/// lane hidden from the preview and the export.
fn lane_state(label: &str, off: bool) -> &'static str {
    match (label == "Audio", off) {
        (true, true) => "muted",
        (true, false) => "on",
        (false, true) => "hidden",
        (false, false) => "shown",
    }
}

/// The slash through an off lane's glyph: 22 long and 1.5 wide at -45°,
/// in the glyph's colour, over the header's middle.
fn lane_slash(color: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let centre = bounds.center();
            let half = Theme::LANE_SLASH_LENGTH / 2.;
            let side = Theme::LANE_SLASH_WIDTH / 2.;
            let diagonal = std::f32::consts::FRAC_1_SQRT_2;
            // Along the slash, bottom left to top right, and across it.
            let along = |t: f32| point(px(t * diagonal), px(-t * diagonal));
            let across = |t: f32| point(px(t * diagonal), px(t * diagonal));
            let mut path = PathBuilder::fill();
            path.move_to(centre - along(half) - across(side));
            path.line_to(centre + along(half) - across(side));
            path.line_to(centre + along(half) + across(side));
            path.line_to(centre - along(half) + across(side));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// How short and how tall the console's top edge can drag the lane region:
/// down to the ruler and two lanes, and up to half the window. The top
/// is not held to the lanes the project has, so a console at rest on a short
/// project still drags taller, and room above the lanes waits for new ones.
pub(super) fn lane_stack_range(window: &Window) -> (f32, f32) {
    let share = f32::from(window.viewport_size().height) * Theme::LANE_STACK_MAX_SHARE;
    (Theme::LANE_STACK_MIN, share.max(Theme::LANE_STACK_MIN))
}

impl RootView {
    /// A strip along a float's edge that takes a drag to resize it, with a
    /// grip that shows under the pointer and while the drag runs. A double
    /// click puts the float back at its resting size.
    ///
    /// Not drawn by the design: the handoff's floats are fixed, and it draws
    /// no grip.
    pub(super) fn resize_edge(&self, edge: ResizeEdge, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let id: SharedString = match edge {
            ResizeEdge::Console => "resize-console".into(),
            ResizeEdge::Inspector => "resize-inspector".into(),
        };
        let hover = subtake_ui::motion::tween_key(&id.clone().into(), "hover");
        let dragging = matches!(
            self.gesture,
            Some(Gesture::Resize { edge: active, .. }) if active == edge
        );
        let grip = theme.muted.opacity(0.5);
        let grip = if dragging {
            grip
        } else {
            subtake_ui::motion::hover_blend(&hover, grip.opacity(0.), grip)
        };
        let across = edge == ResizeEdge::Console;
        div()
            .id(id)
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .when(across, |el| {
                el.top_0()
                    .left_0()
                    .right_0()
                    .h(px(Theme::RESIZE_HANDLE))
                    .cursor(CursorStyle::ResizeUpDown)
            })
            .when(!across, |el| {
                el.left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(Theme::RESIZE_HANDLE))
                    .cursor(CursorStyle::ResizeLeftRight)
            })
            .on_hover(subtake_ui::motion::hover_listener(hover))
            .child(
                div()
                    .rounded_full()
                    .bg(grip)
                    .when(across, |el| {
                        el.w(px(Theme::RESIZE_GRIP_LENGTH))
                            .h(px(Theme::RESIZE_GRIP_WIDTH))
                    })
                    .when(!across, |el| {
                        el.w(px(Theme::RESIZE_GRIP_WIDTH))
                            .h(px(Theme::RESIZE_GRIP_LENGTH))
                    }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, event: &MouseDownEvent, _, cx| {
                    let rest = event.click_count >= 2;
                    let (origin, start) = match edge {
                        ResizeEdge::Console => {
                            if rest {
                                s.lane_height = Theme::LANE_STACK_HEIGHT;
                            }
                            (f32::from(event.position.y), s.lane_height)
                        }
                        ResizeEdge::Inspector => {
                            if rest {
                                s.inspector_width = PANEL_WIDTH;
                            }
                            (f32::from(event.position.x), s.inspector_width)
                        }
                    };
                    if !rest {
                        s.gesture = Some(Gesture::Resize {
                            edge,
                            origin,
                            start,
                        });
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
    }

    /// Follows a drag wherever the pointer goes. The floats occlude what is
    /// behind them, so a listener on the root lost the pointer the moment it
    /// crossed onto one: a drag held only while it stayed over the stage.
    /// The window's own mouse events reach this whatever is under them.
    pub(super) fn gesture_follower(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let moved = cx.listener(Self::move_gesture);
        let released = cx.listener(Self::end_gesture);
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture {
                        moved(event, window, cx);
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        released(event, window, cx);
                    }
                });
            },
        )
        .absolute()
        .inset_0()
    }

    pub(super) fn seek_at(&self, x: Pixels) {
        if let Surface::Editor(e) = &self.surface {
            let b = self.timeline_bounds.get();
            let fraction = f32::from(x - b.left()) / f32::from(b.size.width).max(1.);
            e.invoke_seek(
                (e.get_timeline_offset() + fraction * e.get_timeline_visible())
                    .clamp(0., e.get_duration()),
            );
        }
    }

    pub(super) fn move_gesture(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Surface::Editor(e) = &self.surface else {
            return;
        };
        match &mut self.gesture {
            Some(Gesture::Resize {
                edge,
                origin,
                start,
            }) => match edge {
                ResizeEdge::Console => {
                    let (min, max) = lane_stack_range(window);
                    self.lane_height =
                        (*start + *origin - f32::from(event.position.y)).clamp(min, max);
                }
                ResizeEdge::Inspector => {
                    self.inspector_width = (*start + *origin - f32::from(event.position.x))
                        .clamp(PANEL_WIDTH_MIN, PANEL_WIDTH_MAX);
                }
            },
            Some(Gesture::Seek { grab }) => {
                let at = event.position.x - px(*grab);
                self.seek_at(at)
            }
            Some(Gesture::Region { origin, delta, .. }) => {
                *delta = f32::from(event.position.x - origin.x)
                    / f32::from(self.timeline_bounds.get().size.width).max(1.)
                    * e.get_timeline_visible();
            }
            Some(Gesture::Canvas { origin, dx, dy, .. }) => {
                let b = self.preview_bounds.get();
                *dx = f32::from(event.position.x - origin.x) / f32::from(b.size.width).max(1.);
                *dy = f32::from(event.position.y - origin.y) / f32::from(b.size.height).max(1.);
            }
            None => return,
        }
        cx.notify();
    }

    pub(super) fn end_gesture(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(gesture) = self.gesture.take() {
            if let Surface::Editor(e) = &self.surface {
                match gesture {
                    Gesture::Seek { grab } => self.seek_at(event.position.x - px(grab)),
                    Gesture::Region {
                        region,
                        origin,
                        mode,
                        ..
                    } => {
                        let delta = f32::from(event.position.x - origin.x)
                            / f32::from(self.timeline_bounds.get().size.width).max(1.)
                            * e.get_timeline_visible();
                        if delta.abs() > 0.00001 {
                            e.invoke_move_region(region.kind, region.id, delta, mode);
                        }
                    }
                    Gesture::Resize { .. } => {}
                    Gesture::Canvas { origin, resize, .. } => {
                        let b = self.preview_bounds.get();
                        e.invoke_canvas_edit(
                            f32::from(event.position.x - origin.x)
                                / f32::from(b.size.width).max(1.),
                            f32::from(event.position.y - origin.y)
                                / f32::from(b.size.height).max(1.),
                            resize,
                        );
                    }
                }
            }
            cx.notify();
        }
    }

    pub(super) fn timeline(
        &mut self,
        window: &EditorWindow,
        win: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let visible = window.get_timeline_visible();
        let offset = window.get_timeline_offset();
        let editor = window.clone();
        let editor_out = window.clone();
        let editor_in = window.clone();
        let (elapsed, total) = window
            .get_time_label()
            .split_once(" / ")
            .map(|(a, b)| (a.to_owned(), format!(" / {b}")))
            .unwrap_or_else(|| (window.get_time_label(), String::new()));
        let toolbar = row()
            .gap(px(Theme::GAP_LARGE))
            // The transport belongs to the console, not to the stage. It had
            // been floated over the picture on a pod of its own, which is the
            // one thing the handoff does NOT float: the pods carry the tools
            // and the framing, and the thing that moves the playhead sits on
            // the same surface as the playhead.
            .child(
                row()
                    .gap(px(Theme::GAP_LARGE))
                    .flex_none()
                    .child(
                        row()
                            .gap(px(Theme::GAP_SMALL))
                            .child(self.icon_action(
                                "previous-frame",
                                "SkipBack-fill",
                                "Previous frame",
                                "previous-frame",
                                true,
                            ))
                            .child(
                                icon_button(
                                    "play",
                                    if window.get_playing() {
                                        "Pause-fill"
                                    } else {
                                        "Play-fill"
                                    },
                                    if window.get_playing() {
                                        "Pause"
                                    } else {
                                        "Play"
                                    },
                                    theme,
                                )
                                .transport()
                                .on_click(self.command("play")),
                            )
                            .child(self.icon_action(
                                "next-frame",
                                "SkipForward-fill",
                                "Next frame",
                                "next-frame",
                                true,
                            )),
                    )
                    // Geist Mono, not Geist. A timecode counts, and
                    // proportional digits reflow as it does — every glyph
                    // beside the seconds shifted each time they ticked from 9
                    // to 10. The position is the reading and the duration is
                    // the context, so only the position is at full strength.
                    .child(
                        mono(elapsed)
                            .flex_none()
                            .text_size(px(Theme::FONT_TIMECODE))
                            .text_color(theme.text)
                            .child(
                                div()
                                    .text_size(px(Theme::FONT_TIMECODE))
                                    .text_color(theme.muted)
                                    .child(total),
                            )
                            .flex()
                            .items_baseline(),
                    ),
            )
            .child(div().flex_1())
            .child(
                row()
                    .gap(px(Theme::GAP))
                    .flex_none()
                    .child(
                        button("auto-zoom", "Suggest zooms", theme)
                            .glyph("MagicWand-regular")
                            .raised()
                            .on_click(self.command("auto-zoom")),
                    )
                    .child(
                        self.action("split-clip", "Split", "split-clip", true)
                            .glyph("Scissors-regular")
                            .raised(),
                    )
                    .child(self.menu_button("Add", cx)),
            )
            .child(div().flex_1())
            // Snap is an on/off setting, so it takes a `sunk` plate while
            // engaged, not the accent; the zoom is the stage's own control,
            // so the two zooms read alike.
            .child(
                row()
                    .gap(px(Theme::GAP_SMALL))
                    .flex_none()
                    .child(
                        icon_button("snap", "Magnet-regular", "Snap", theme)
                            .ghost()
                            .toggled(window.get_snap())
                            .on_click(move |_, _, _| editor.set_snap(!editor.get_snap())),
                    )
                    .child(
                        zoom_control("timeline-zoom", window.get_timeline_zoom(), theme)
                            .can_zoom_out(window.get_timeline_zoom() > 1.)
                            .can_zoom_in(window.get_timeline_zoom() < TIMELINE_ZOOM_MAX)
                            .can_fit(window.get_timeline_zoom() > 1.)
                            .on_zoom_out(move |_, _, _| {
                                editor_out.set_timeline_zoom(
                                    (editor_out.get_timeline_zoom() / TIMELINE_ZOOM_STEP).max(1.),
                                )
                            })
                            .on_zoom_in(move |_, _, _| {
                                editor_in.set_timeline_zoom(
                                    (editor_in.get_timeline_zoom() * TIMELINE_ZOOM_STEP)
                                        .min(TIMELINE_ZOOM_MAX),
                                )
                            })
                            .on_fit(cx.listener(|s, _, _, _| {
                                if let Surface::Editor(window) = &s.surface {
                                    window.set_timeline_zoom(1.);
                                    window.set_timeline_offset(0.);
                                }
                            })),
                    ),
            );
        let regions: Vec<Region> = window.get_regions().iter().collect();
        // The gallery's held states: the selected region moved or its end
        // trimmed a few seconds on, or the playhead held. A pointer moving
        // over the window carries the hold on from where it is.
        if let Some(hold) = self.gallery_gesture.take() {
            let selected = regions.iter().find(|r| r.selected).cloned();
            self.gesture = match (hold.as_str(), selected) {
                ("scrub", _) => Some(Gesture::Seek { grab: 0. }),
                ("move" | "trim", Some(region)) => Some(Gesture::Region {
                    region,
                    origin: point(px(0.), px(0.)),
                    mode: i32::from(hold == "trim"),
                    delta: GALLERY_HOLD_SECONDS,
                }),
                _ => None,
            };
        }
        let track_width = f32::from(self.timeline_bounds.get().size.width);
        let playhead_time = window.get_playhead();
        let playhead = (playhead_time - offset) / visible;
        let playhead_shown = (0. ..=1.).contains(&playhead);
        let scrubbing = matches!(self.gesture, Some(Gesture::Seek { .. }));
        // The ruler: a recessed band with a label every major interval and a
        // dot at every fifth of one between them. The major interval is the
        // smallest that keeps its labels 80 apart, so zooming in picks a
        // finer one. What the playhead has passed is at full strength and
        // what is ahead of it muted, every frame. The playhead's bubble sits
        // above the band, so no label needs hiding for it. A label that
        // would run off either end of the band is left out.
        let interval = RULER_INTERVALS
            .into_iter()
            .find(|seconds| seconds / visible * track_width >= Theme::RULER_LABEL_SPACING)
            .unwrap_or(RULER_INTERVALS[RULER_INTERVALS.len() - 1]);
        let minor = interval / Theme::RULER_MINOR_STEPS;
        let mut ruler = div()
            .id("ruler")
            .relative()
            .h(px(Theme::RULER_HEIGHT))
            .rounded_full()
            .bg(theme.sunk)
            .shadow(vec![hairline(theme.line, Theme::HAIRLINE_WIDTH)]);
        if track_width > 0. {
            let first = (offset / minor).ceil() as i64;
            let last = ((offset + visible) / minor).floor() as i64;
            for tick in first..=last {
                let seconds = tick as f32 * minor;
                let x = (seconds - offset) / visible * track_width;
                let past = seconds <= playhead_time;
                if tick % Theme::RULER_MINOR_STEPS as i64 == 0 {
                    let label = ruler_clock(seconds, false);
                    let width =
                        mono_width(&label, Theme::FONT_SMALL) + 2. * Theme::RULER_LABEL_PADDING;
                    if x - width / 2. >= 0. && x + width / 2. <= track_width {
                        ruler = ruler.child(
                            mono(label)
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(px(x - width / 2.))
                                .w(px(width))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(Theme::FONT_SMALL))
                                .text_color(if past { theme.text } else { theme.muted }),
                        );
                    }
                } else {
                    // Not drawn by the design: a dot that would touch the
                    // label beside it is left out. At the closest spacing
                    // the design allows the dot either side of a label ran
                    // into its text.
                    let steps = Theme::RULER_MINOR_STEPS as i64;
                    let near = (tick as f32 / steps as f32).round() * interval;
                    let half = mono_width(&ruler_clock(near, false), Theme::FONT_SMALL) / 2.
                        + Theme::RULER_LABEL_PADDING;
                    if ((seconds - near) / visible * track_width).abs() < half + Theme::RULER_DOT {
                        continue;
                    }
                    let dot = if past {
                        theme.text.opacity(Theme::RULER_DOT_PAST)
                    } else {
                        theme.muted.opacity(Theme::RULER_DOT_AHEAD)
                    };
                    ruler = ruler.child(
                        div()
                            .absolute()
                            .left(px(x - Theme::RULER_DOT / 2.))
                            .top(px((Theme::RULER_HEIGHT - Theme::RULER_DOT) / 2.))
                            .size(px(Theme::RULER_DOT))
                            .rounded_full()
                            .bg(dot),
                    );
                }
            }
        }
        let labels: Vec<String> = window.get_track_labels().iter().collect();
        // The lanes drawn: every row with something on it, in order. A row
        // with nothing on it is not drawn at all, header included; adding a
        // region of its kind brings it back. Where each drawn lane starts
        // comes from here, so a lane, its header and the blocks on it cannot
        // drift apart.
        let shown: Vec<usize> = (0..labels.len())
            .filter(|&row| regions.iter().any(|r| r.row as usize == row))
            .collect();
        let mut tops = vec![None; labels.len()];
        let mut stack = 0.;
        for &row in &shown {
            tops[row] = Some(stack);
            stack += lane_height(&labels[row]) + Theme::LANE_GAP;
        }
        let stack = (stack - Theme::LANE_GAP).max(0.);
        // The recording's frames, cut once per strip rather than per frame.
        let frames = match (window.get_thumbnails().0, &self.clip_frames) {
            (Some(strip), Some((cut, frames))) if Arc::ptr_eq(&strip, cut) => frames.clone(),
            (Some(strip), _) => {
                let frames = split_frames(&strip);
                self.clip_frames = Some((strip, frames.clone()));
                frames
            }
            (None, _) => Vec::new(),
        };
        let peaks = match (window.get_waveform().0, &self.wave_peaks) {
            (Some(image), Some((read, peaks))) if Arc::ptr_eq(&image, read) => peaks.clone(),
            (Some(image), _) => {
                let peaks = read_peaks(&image);
                self.wave_peaks = Some((image, peaks.clone()));
                peaks
            }
            (None, _) => Arc::from([]),
        };
        let duration = window.get_duration();
        let mut tracks = div().relative().h(px(stack));
        for region in regions.iter() {
            let Some(top) = tops.get(region.row as usize).copied().flatten() else {
                continue;
            };
            let height = lane_height(&labels[region.row as usize]);
            let take = region.is_take();
            // An off lane's regions show at 40%, and still select and edit.
            let lane_off = self.lanes_off.contains(&labels[region.row as usize]);
            let (mut start, mut end) = (region.start, region.end);
            if let Some(Gesture::Region {
                region: dragged,
                delta,
                mode,
                ..
            }) = &self.gesture
                && dragged.id == region.id
                && dragged.kind == region.kind
            {
                if *mode != 1 {
                    start += delta;
                }
                if *mode != 2 {
                    end += delta;
                }
            }
            // A pill the lane's height in a solid fill and an ink from the
            // lane's hue, with no edge at rest. Selected, it gains a 2px
            // accent ring and its trim handles. Dragged, it lifts on the
            // panel shadow over a dashed outline where it started.
            let (fill, ink) = theme.region_tones(region.tint.to_gpui());
            let moving = matches!(
                &self.gesture,
                Some(Gesture::Region { region: dragged, mode: 0, .. })
                    if dragged.id == region.id && dragged.kind == region.kind
            );
            let width = (track_width * (end - start) / visible).max(Theme::GAP);
            let clip = matches!(region.kind.as_str(), "clipRegions" | Region::TAKE_CLIP);
            if moving
                && let Some((ghost, _, _)) = tucked(
                    div().absolute(),
                    (region.start, region.end, top, height),
                    (offset, visible, track_width),
                )
            {
                tracks = tracks.child(
                    ghost
                        .rounded_full()
                        .border(px(Theme::REGION_GHOST_WIDTH))
                        .border_dashed()
                        .border_color(theme.line),
                );
            }
            let block_id: ElementId =
                SharedString::from(format!("region-{}-{}", region.kind, region.id)).into();
            let hover_key = subtake_ui::motion::tween_key(&block_id, "hover");
            let mark_key = hover_key.clone();
            let ring = subtake_ui::focus_ring(theme);
            let Some((block, cut, inset)) = tucked(
                div().id(block_id).absolute(),
                (start, end, top, height),
                (offset, visible, track_width),
            ) else {
                continue;
            };
            let block = block
                .rounded_full()
                .bg(fill)
                .when(lane_off, |el| el.opacity(Theme::LANE_OFF_ALPHA))
                .on_hover(subtake_ui::motion::hover_listener(hover_key))
                .when(moving, |el| el.shadow(theme.panel_shadow()))
                // Held, a region dims as every pressed control does, and
                // stays dimmed while it is dragged. Tab reaches it, and Enter
                // or Space selects it, which is what a click does.
                .when(!take, |el| {
                    el.active(|s| s.opacity(Theme::PRESSED_OPACITY))
                        .tab_index(0)
                        .focus_visible(move |s| s.shadow(vec![ring]))
                        .cursor(CursorStyle::ClosedHand)
                });
            // What the region shows, at its full length and cut where the
            // region is: a region tucked under the headers keeps its label
            // and frames where they were rather than sliding them along.
            // A clip's frames are its fill, cut with it at the round end;
            // what shows over the fill is cut at the header's centre,
            // `centre` into the region.
            let centre = if cut > 0. { height / 2. } else { 0. };
            let sheet = |at: f32| {
                div()
                    .absolute()
                    .top(px(-inset))
                    .h(px(height))
                    .when(cut > 0., |el| el.left(px(-(cut + at))).w(px(width)))
                    .when(cut <= 0., |el| el.left_0().w_full())
            };
            let cut_at = |at: f32, sheet: Div| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(at))
                    .right_0()
                    .overflow_hidden()
                    .child(sheet)
            };
            // A clip is its frames under a label chip; the tint shows while
            // they load. Not drawn by the design: each tile's frame is the
            // recording's at the tile's place on the timeline, not at its
            // place in the clip's source.
            let frames = clip.then(|| {
                let left = track_width * (start - offset) / visible;
                cut_at(
                    0.,
                    sheet(0.).children(clip_tiles(
                        &frames,
                        (start, end),
                        (left, width, height, cut),
                        (track_width, duration),
                    )),
                )
            });
            let over = sheet(centre)
                .when(clip, |el| {
                    el.children(clip_chip(&region.label, width, theme))
                })
                .when(!clip, |el| {
                    el.child({
                        // The kind's icon on a `plate` circle, then the label in
                        // the ink. A label that would get fewer than four
                        // letters is left off rather than cut to a fragment, and
                        // the region shows its plate alone, 4 in at both ends;
                        // the tooltip names it either way. Not drawn by the
                        // design: a region too short for its plate shrinks the
                        // plate to fit, and drops the icon once the plate is
                        // smaller than it.
                        let icon = region_icon(region);
                        let inset = Theme::REGION_PLATE_INSET;
                        let plate = (height - 2. * inset).min(width - 2. * inset).max(0.);
                        let room = width
                            - Theme::REGION_PADDING_START
                            - icon.map_or(0., |_| plate + Theme::REGION_GAP)
                            - Theme::REGION_PADDING_END;
                        let labelled = room >= Theme::REGION_LABEL_MIN_ROOM;
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .gap(px(Theme::REGION_GAP))
                            .when(labelled, |el| {
                                el.pl(px(Theme::REGION_PADDING_START))
                                    .pr(px(Theme::REGION_PADDING_END))
                            })
                            .when(!labelled, |el| el.justify_center())
                            .text_size(px(Theme::FONT_BODY))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ink)
                            .when_some(icon, |el, name| {
                                el.child(
                                    div()
                                        .flex_none()
                                        .size(px(plate))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(theme.plate)
                                        .when(plate >= Theme::REGION_ICON_SIZE, |el| {
                                            el.child(subtake_ui::icon_sized(
                                                name,
                                                Theme::REGION_ICON_SIZE,
                                                ink,
                                            ))
                                        }),
                                )
                            })
                            // On one line, in a box that may shrink below it: a
                            // flex row gives bare text its full width, and a
                            // short region cut its label off mid-letter instead.
                            .when(labelled, |el| {
                                el.child(
                                    div()
                                        .min_w_0()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(region.label.clone()),
                                )
                            })
                            // The recording's sound fills the rest. Not
                            // wired: an imported sound's waveform, which is
                            // never read.
                            .when(labelled && region.kind == Region::TAKE_AUDIO, |el| {
                                el.child(waveform(
                                    peaks.clone(),
                                    self.timeline_bounds.clone(),
                                    (offset, visible, duration, playhead_time),
                                    ink,
                                ))
                            })
                    })
                });
            let mut block = block.children(frames).child(cut_at(centre, over));
            // The selected ring, over the fill and under the handles: a
            // bordered overlay, not the block's own border, which would shift
            // its label by the ring's width; and not an inset shadow, which
            // gpui paints under the element's own fill, so an opaque region
            // hid it. In its own layer, as the handles are, since the console
            // is one layer and in one layer a clip's frames paint over every
            // fill and border whatever order they come in.
            if region.selected {
                block = block.child(layered(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded_full()
                        .border(px(Theme::REGION_SELECTED_RING))
                        .border_color(theme.accent),
                ));
            }
            // The tooltip names the region and gives its range, for one
            // shown as an icon or with its label cut short. Not drawn by the
            // design: it shows on every region, since whether a label is cut
            // is only known after layout, and it sits under the pointer
            // rather than 8 above the region, centred, as gpui places a
            // tooltip.
            if !region.label.is_empty() {
                let label = region.label.clone();
                let range = format!(
                    "{}\u{2013}{}",
                    ruler_clock(region.start, false),
                    ruler_clock(region.end, false)
                );
                block = block.tooltip(move |_, cx| {
                    subtake_ui::tooltip_detail(label.clone(), range.clone(), theme, cx)
                });
            }
            // The take's own clip and sound take no selection and no drag:
            // a press on one seeks, as a press on the bare lane does.
            if take {
                tracks = tracks.child(block);
                continue;
            }
            let key_region = region.clone();
            block = block.on_click(cx.listener(move |s, event: &ClickEvent, _, cx| {
                if !event.is_keyboard() {
                    return;
                }
                if let Surface::Editor(window) = &s.surface {
                    window.invoke_select_region(
                        key_region.kind.clone(),
                        key_region.id.clone(),
                        false,
                    );
                }
                cx.notify();
            }));
            let drag_region = region.clone();
            block = block.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    if let Surface::Editor(window) = &s.surface {
                        window.invoke_select_region(
                            drag_region.kind.clone(),
                            drag_region.id.clone(),
                            event.modifiers.shift,
                        );
                    }
                    s.gesture = Some(Gesture::Region {
                        region: drag_region.clone(),
                        origin: event.position,
                        mode: 0,
                        delta: 0.,
                    });
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
            for (mode, right) in [(2, false), (1, true)] {
                // A start tucked under the headers has no handle: it is
                // under a header, out of reach.
                if !right && cut > 0. {
                    continue;
                }
                let drag_region = region.clone();
                // An accent bar with a white ring standing 3 out past the
                // region's end, in a wider grab area centred on it. It shows
                // when the region is selected, at half strength under the
                // pointer, and grows to 6 × 22 at full strength while it is
                // dragged — a timeline of twenty regions showing forty
                // handles is a texture, not a set of controls.
                let held = matches!(
                    &self.gesture,
                    Some(Gesture::Region { region: dragged, mode: held, .. })
                        if *held == mode && dragged.id == region.id && dragged.kind == region.kind
                );
                let shown = f32::from(region.selected || held);
                let hovered = shown.max(Theme::REGION_HANDLE_HOVER);
                let (mark_width, mark_height) = if held {
                    (
                        Theme::REGION_HANDLE_WIDTH_HELD,
                        Theme::REGION_HANDLE_HEIGHT_HELD,
                    )
                } else {
                    (Theme::REGION_HANDLE_WIDTH, Theme::REGION_HANDLE_HEIGHT)
                };
                let white = gpui::white();
                let centre = Theme::REGION_HANDLE_WIDTH / 2. - Theme::REGION_HANDLE_OUTSET;
                let mut handle = div()
                    .id(("resize", mode as usize))
                    .absolute()
                    .top_0()
                    .w(px(Theme::REGION_HANDLE_TARGET))
                    .h_full()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .child(
                        div()
                            .absolute()
                            .left(px((Theme::REGION_HANDLE_TARGET - mark_width) / 2.))
                            .top(px((height - mark_height) / 2.))
                            .w(px(mark_width))
                            .h(px(mark_height))
                            .rounded(px(Theme::REGION_HANDLE_RADIUS))
                            .bg(subtake_ui::motion::hover_blend(
                                &mark_key,
                                theme.accent.opacity(shown),
                                theme.accent.opacity(hovered),
                            ))
                            .shadow(vec![BoxShadow {
                                color: subtake_ui::motion::hover_blend(
                                    &mark_key,
                                    white.opacity(shown),
                                    white.opacity(hovered),
                                ),
                                offset: point(px(0.), px(0.)),
                                blur_radius: px(0.),
                                spread_radius: px(Theme::REGION_HANDLE_RING),
                                inset: false,
                            }]),
                    );
                let edge = px(centre - Theme::REGION_HANDLE_TARGET / 2.);
                handle = if right {
                    handle.right(edge)
                } else {
                    handle.left(edge)
                };
                block = block.child(layered(handle.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |s, event: &MouseDownEvent, w, cx| {
                        w.focus(&s.focus, cx);
                        if let Surface::Editor(window) = &s.surface {
                            window.invoke_select_region(
                                drag_region.kind.clone(),
                                drag_region.id.clone(),
                                event.modifiers.shift,
                            );
                        }
                        s.gesture = Some(Gesture::Region {
                            region: drag_region.clone(),
                            origin: event.position,
                            mode,
                            delta: 0.,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )));
            }
            tracks = tracks.child(block);
        }
        let mut timeline = column()
            .id("timeline-content")
            .relative()
            .flex_1()
            .min_w_0()
            .pt(px(Theme::BUBBLE_ZONE))
            .gap(px(Theme::RULER_GAP))
            .child(measure(self.timeline_bounds.clone()))
            .child(ruler)
            .child(tracks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    s.gesture = Some(Gesture::Seek { grab: 0. });
                    s.seek_at(event.position.x);
                    cx.stop_propagation();
                }),
            );
        if playhead_shown {
            let x = playhead * track_width;
            // Where the grab handle rides: centred on the clip lane, or on
            // the first lane when there is no clip lane. Not drawn by the
            // design, which always has one.
            let handle_lane = shown
                .iter()
                .copied()
                .find(|&row| labels[row] == "Clip")
                .or(shown.first().copied());
            let handle_top = handle_lane.map(|row| {
                Theme::LANE_STACK_TOP
                    + tops[row].unwrap_or(0.)
                    + (lane_height(&labels[row]) - Theme::PLAYHEAD_HANDLE_HEIGHT) / 2.
            });
            let chip_label = ruler_clock(playhead_time, true);
            let bubble_width = mono_width(&chip_label, Theme::FONT_SECONDARY)
                + 2. * Theme::PLAYHEAD_BUBBLE_PADDING;
            let bubble_left =
                (x - bubble_width / 2.).clamp(0., (track_width - bubble_width).max(0.));
            let handle_width = if scrubbing {
                Theme::PLAYHEAD_HANDLE_WIDTH_HELD
            } else {
                Theme::PLAYHEAD_HANDLE_WIDTH
            };
            // A press on any part of the playhead picks it up where it is.
            let grab = || {
                cx.listener(move |s: &mut Self, event: &MouseDownEvent, w, cx| {
                    w.focus(&s.focus, cx);
                    let b = s.timeline_bounds.get();
                    let at = f32::from(b.left()) + x;
                    s.gesture = Some(Gesture::Seek {
                        grab: f32::from(event.position.x) - at,
                    });
                    cx.stop_propagation();
                    cx.notify();
                })
            };
            // Bottom to top: the line over the regions, the dot on the
            // ruler's top edge, the bubble with its tail, and the handle.
            // The line keeps a 12-wide hit area that scrubs and shows the
            // resize cursor, since 1.5 is not a target. The line and the
            // handle paint in layers of their own, over a clip's frames.
            timeline = timeline
                .child(layered(
                    div()
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_LINE_WIDTH / 2.))
                        .top(px(Theme::BUBBLE_ZONE))
                        .bottom_0()
                        .w(px(Theme::PLAYHEAD_LINE_WIDTH))
                        .rounded(px(Theme::PLAYHEAD_LINE_RADIUS))
                        .bg(theme.accent)
                        .shadow(vec![glow(theme, Theme::PLAYHEAD_LINE_GLOW)]),
                ))
                .child(
                    div()
                        .id("playhead-line")
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_HIT))
                        .top(px(Theme::BUBBLE_ZONE))
                        .bottom_0()
                        .w(px(Theme::PLAYHEAD_HIT * 2.))
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .child(
                    div()
                        .id("playhead-dot")
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_DOT / 2.))
                        .top(px(Theme::BUBBLE_ZONE - Theme::PLAYHEAD_DOT / 2.))
                        .size(px(Theme::PLAYHEAD_DOT))
                        .rounded_full()
                        .bg(theme.accent)
                        .shadow(vec![BoxShadow {
                            color: theme.accent_soft,
                            offset: point(px(0.), px(0.)),
                            blur_radius: px(0.),
                            spread_radius: px(Theme::PLAYHEAD_DOT_RING),
                            inset: false,
                        }])
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .child(
                    playhead_tail(theme.accent)
                        .absolute()
                        .left(px(x - Theme::PLAYHEAD_TAIL_WIDTH / 2.))
                        .top(px(Theme::PLAYHEAD_BUBBLE_HEIGHT))
                        .w(px(Theme::PLAYHEAD_TAIL_WIDTH))
                        .h(px(Theme::PLAYHEAD_TAIL_HEIGHT)),
                )
                // Held inside the track at either end rather than cut off by
                // it; the tail stays on the true time.
                .child(
                    mono(chip_label)
                        .id("playhead-bubble")
                        .absolute()
                        .top_0()
                        .left(px(bubble_left))
                        .w(px(bubble_width))
                        .h(px(Theme::PLAYHEAD_BUBBLE_HEIGHT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(theme.accent)
                        .shadow(vec![glow(
                            theme,
                            if scrubbing {
                                Theme::PLAYHEAD_BUBBLE_GLOW_HELD
                            } else {
                                Theme::PLAYHEAD_BUBBLE_GLOW
                            },
                        )])
                        .text_color(theme.on_accent)
                        .text_size(px(Theme::FONT_SECONDARY))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(MouseButton::Left, grab()),
                )
                .when_some(handle_top, |el, top| {
                    el.child(layered(
                        div()
                            .id("playhead-handle")
                            .absolute()
                            .left(px(x - handle_width / 2.))
                            .top(px(top))
                            .w(px(handle_width))
                            .h(px(Theme::PLAYHEAD_HANDLE_HEIGHT))
                            .rounded(px(Theme::PLAYHEAD_HANDLE_RADIUS))
                            .bg(theme.accent)
                            .shadow(vec![glow(
                                theme,
                                if scrubbing {
                                    Theme::PLAYHEAD_HANDLE_GLOW_HELD
                                } else {
                                    Theme::PLAYHEAD_HANDLE_GLOW
                                },
                            )])
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(Theme::PLAYHEAD_GRIP_GAP))
                            .cursor(CursorStyle::ResizeLeftRight)
                            .children((0..3).map(|_| {
                                div()
                                    .size(px(Theme::PLAYHEAD_GRIP_DOT))
                                    .rounded_full()
                                    .bg(theme.on_accent)
                            }))
                            .on_mouse_down(MouseButton::Left, grab()),
                    ))
                });
        }
        let headers: Vec<Div> = shown
            .iter()
            .map(|&row| {
                let label = labels[row].clone();
                let first = row == 0 || labels[row - 1] != label;
                if !first {
                    return div().h(px(lane_height(&label)));
                }
                let off = self.lanes_off.contains(&label);
                let toggled = label.clone();
                lane_header(
                    &label,
                    off,
                    theme,
                    cx.listener(move |s, _: &ClickEvent, _, cx| {
                        if !s.lanes_off.remove(&toggled) {
                            s.lanes_off.insert(toggled.clone());
                        }
                        cx.notify();
                    }),
                )
            })
            .collect();
        let console = panel(theme)
            .id("timeline")
            .relative()
            // A zoomed picture runs on under the console; the console's
            // presses are its own.
            .occlude()
            .mx(px(Theme::INSET))
            .mb(px(Theme::INSET))
            .flex_shrink_0()
            .child(toolbar)
            // The lanes and the export/transcription line share one box, so
            // the line folds away without leaving the console's gap behind.
            // It sits inside the console rather than under it so the console
            // keeps the shell's own inset on all three of its edges.
            .child(
                column().gap_0().flex_none().child(
                    fade_edges(
                        // Zoomed in, the lanes run on past the track column:
                        // out to the console's own edge on the right, and
                        // under the headers on the left, as a region's pill
                        // runs under its plate. The ruler keeps to the
                        // track column.
                        row()
                            .id("track-scroll")
                            .relative()
                            .items_start()
                            .ml(px(-Theme::LANE_TUCK_OUTSET))
                            .pl(px(Theme::LANE_TUCK_OUTSET
                                + Theme::LANE_HEADER
                                + Theme::LANE_HEADER_GAP))
                            .mr(px(-Theme::PANEL_PADDING))
                            .pr(px(Theme::PANEL_PADDING))
                            .overflow_x_hidden()
                            .h(px({
                                let (min, max) = lane_stack_range(win);
                                self.lane_height.clamp(min, max)
                            }))
                            .flex_none()
                            .overflow_y_scroll()
                            .track_scroll(&self.lane_scroll)
                            .child(timeline)
                            // The headers run on the track column's own grid,
                            // row for row: down past the bubble's band, the
                            // ruler and the air under it, then one row per
                            // lane at that lane's height with the same gap.
                            // A lane that overflows onto a second row shares
                            // the first row's header. Not drawn by the
                            // design, which gives every lane one row. They
                            // come after the lanes, in a layer of their own,
                            // so they sit over a clip's frames run under them.
                            .child(layered(
                                column()
                                    .absolute()
                                    .left(px(Theme::LANE_TUCK_OUTSET))
                                    .top_0()
                                    .w(px(Theme::LANE_HEADER))
                                    .flex_shrink_0()
                                    .gap(px(Theme::LANE_GAP))
                                    .pt(px(Theme::LANE_STACK_TOP))
                                    .children(headers),
                            )),
                    )
                    // The top fades by what is scrolled past it, so a
                    // stack that fits ends at its last lane rather than
                    // on a band of air kept for the fade to rest on. The
                    // bottom cuts hard: the console's own edge already
                    // closes it.
                    .tracking(&self.lane_scroll)
                    .bottom(false),
                ),
            )
            // Its top edge takes a drag, trading lane height for stage.
            .child(self.resize_edge(ResizeEdge::Console, cx))
            .on_scroll_wheel(cx.listener(|s, event: &ScrollWheelEvent, _, cx| {
                if let Surface::Editor(window) = &s.surface {
                    let delta = event.delta.pixel_delta(px(20.));
                    if event.modifiers.control || event.modifiers.platform {
                        let b = s.timeline_bounds.get();
                        let fraction = (f32::from(event.position.x - b.left())
                            / f32::from(b.size.width).max(1.))
                        .clamp(0., 1.);
                        let anchor =
                            window.get_timeline_offset() + fraction * window.get_timeline_visible();
                        window.set_timeline_zoom(
                            (window.get_timeline_zoom() * (-f32::from(delta.y) * 0.01).exp())
                                .clamp(1., TIMELINE_ZOOM_MAX),
                        );
                        window.set_timeline_offset(
                            (anchor - fraction * window.get_timeline_visible()).clamp(
                                0.,
                                (window.get_duration() - window.get_timeline_visible()).max(0.),
                            ),
                        );
                        cx.stop_propagation();
                    } else if delta.x != px(0.) || event.modifiers.shift {
                        let dx = if event.modifiers.shift {
                            delta.y
                        } else {
                            delta.x
                        };
                        window.set_timeline_offset(
                            (window.get_timeline_offset()
                                - f32::from(dx)
                                    / f32::from(s.timeline_bounds.get().size.width).max(1.)
                                    * window.get_timeline_visible())
                            .clamp(
                                0.,
                                (window.get_duration() - window.get_timeline_visible()).max(0.),
                            ),
                        );
                        cx.stop_propagation();
                    }
                }
            }))
            .into_any_element();
        frosted(UiSurface::Panel.radius(), UiSurface::Panel.blur(), console).into_any_element()
    }
}
