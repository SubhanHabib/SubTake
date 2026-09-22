//! Ported verbatim from the Zeron reference (`zeronsh/zeron`,
//! `crates/ui/src/frost.rs`, MIT © 2026 Wing), adapted only for SubTake's
//! theme import.
//!
//! [`frosted`] — the frosted-glass float: wraps a popover/dialog card so its
//! ENTIRE subtree paints inside one scene layer (a single draw order) with a
//! backdrop blur painted first.
//!
//! The single layer order is the point: with per-primitive bounds-tree
//! ordering, a hover repaint elsewhere could reassign the card's quads BELOW
//! the blur — washes, dividers, and borders intermittently got snapshotted and
//! blurred away (user reports). Inside one layer the blur/content relationship
//! is structural: blur first, then shadow, tint, border, rows, text.

use gpui::{
    AnyElement, App, Bounds, Corners, EdgeFade, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window, px,
};

use subtake_theme::Theme;

// The blur ladder. A float's blur is chosen by what it is, not by how big it
// is: the further a surface floats from the window, the more of the desktop it
// takes out behind itself. The handoff pairs each figure with a `saturate()`,
// which gpui at the pinned revision has no filter for, so only the blur is
// carried.

/// The tool pod — the shallowest float, and the only one that holds nothing
/// but icons.
pub const POD_BLUR: f32 = 28.0;
/// The console and the inspector, and every menu and popover that opens over
/// them. Shared so a menu opening over a panel does not read as a different
/// material from it.
pub const MENU_BLUR: f32 = 34.0;
pub const PANEL_BLUR: f32 = MENU_BLUR;
/// The recorder bar and a modal dialog: the two surfaces that float over
/// something other than this window's own content.
pub const BAR_BLUR: f32 = 38.0;

/// Frost `child` (a popover card): backdrop-blurred on glass, pass-through on
/// opaque platforms. `corner_radius` must match the card's rounding.
pub fn frosted(corner_radius: f32, blur_radius: f32, child: impl IntoElement) -> Frosted {
    Frosted {
        corner_radius,
        blur_radius,
        child: child.into_any_element(),
    }
}

pub struct Frosted {
    corner_radius: f32,
    blur_radius: f32,
    child: AnyElement,
}

impl Element for Frosted {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if Theme::WINDOW_GLASS_SUPPORTED {
            window.paint_layer(bounds, |window| {
                window.paint_backdrop_blur(
                    bounds,
                    Corners::all(px(self.corner_radius)),
                    px(self.blur_radius),
                );
                self.child.paint(window, cx);
            });
        } else {
            self.child.paint(window, cx);
        }
    }
}

impl IntoElement for Frosted {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Paint `child` in its own scene layer, giving it a fresh draw order above
/// everything painted so far in the enclosing layer.
///
/// Needed for overlays INSIDE a frosted card: the card's single layer means
/// every primitive shares one draw order, and equal orders render grouped by
/// primitive kind (quads, then icons, then images) — so a close button's
/// circle painted "after" a thumbnail still shows up UNDER the image. A
/// nested layer restores the intended stacking.
pub fn layered(child: impl IntoElement) -> Layered {
    Layered {
        child: child.into_any_element(),
    }
}

pub struct Layered {
    child: AnyElement,
}

impl Element for Layered {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_layer(bounds, |window| self.child.paint(window, cx));
    }
}

impl IntoElement for Layered {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

// ---------------------------------------------------------------------------
// scroll-edge fade
// ---------------------------------------------------------------------------

/// The ramp depth used by [`fade_edges`], and the padding a scroll region
/// should carry so the ramp lands on empty space when nothing is clipped.
///
/// The figure itself is a theme metric: a box that reserves height for a
/// scroller has to add the band to its own budget, and that arithmetic
/// happens in `metrics.rs` where the rest of it lives.
pub const FADE_BAND: f32 = Theme::FADE_BAND;

/// Fade `child` out across [`FADE_BAND`] at its top and bottom edges.
///
/// Scroll regions over glass cannot hide their clip line behind a gradient
/// overlay: there is no paintable colour equal to "the blurred desktop behind
/// the window", so an opaque scrim would punch a hole in the frost. The fork's
/// [`Window::with_edge_fade`] instead multiplies each primitive's alpha by a
/// vertical ramp, which composites correctly over anything.
///
/// Pair this with `FADE_BAND` of vertical padding inside the scrolled content:
/// at rest the ramp falls on that padding and nothing is dimmed, and once the
/// content actually overflows, the rows crossing the edge dissolve instead of
/// being sliced in half.
pub fn fade_edges(child: impl IntoElement) -> FadeEdges {
    FadeEdges {
        band: FADE_BAND,
        child: child.into_any_element(),
    }
}

pub struct FadeEdges {
    band: f32,
    child: AnyElement,
}

impl FadeEdges {
    pub fn band(mut self, band: f32) -> Self {
        self.band = band;
        self
    }
}

impl Element for FadeEdges {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // A band taller than half the region would cross-fade the middle.
        let band = self.band.min(f32::from(bounds.size.height) / 2.0).max(0.0);
        let fade = (band > 0.0).then_some(EdgeFade {
            bounds,
            band: px(band),
            band_top: None,
            band_bottom: None,
            top: true,
            bottom: true,
            left: false,
            right: false,
        });
        window.with_edge_fade(fade, |window| self.child.paint(window, cx));
    }
}

impl IntoElement for FadeEdges {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
