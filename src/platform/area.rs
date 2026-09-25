//! The Source card's Area overlay, over `native/AreaOverlay.swift`.

/// Receives the area drawn: its display's id, then its left, top, width and
/// height in points from that display's top-left corner. A width of 0 is a
/// cancel.
pub type AreaDrawn = extern "C" fn(u32, f64, f64, f64, f64);

/// An area to open the overlay with already drawn, in `AreaDrawn`'s terms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AreaSeed {
    pub display: u32,
    pub rect: [f64; 4],
}

/// One palette's colours for the overlay, each RGBA from 0 to 1: accent,
/// its hover, on accent, sunk, its hover, text, muted, frost and line.
pub type AreaColours = [[f64; 4]; 9];

#[cfg(target_os = "macos")]
unsafe extern "C" {
    #[allow(clippy::too_many_arguments)]
    fn subtake_draw_area(
        aspect: f64,
        appearance: i32,
        colours: *const f64,
        sans: *const u8,
        sans_length: isize,
        mono: *const u8,
        mono_length: isize,
        seed_display: u32,
        seed_x: f64,
        seed_y: f64,
        seed_width: f64,
        seed_height: f64,
        drawn: AreaDrawn,
    );
}

/// Opens the overlay over every display for an area to be drawn, held to
/// `aspect` (width over height) unless it is `None`. It is drawn in the
/// `light` or the `dark` palette as `appearance` says — `"light"`, `"dark"`
/// or the system's. `drawn` runs once, on the main thread, with the area
/// or a cancel.
pub fn draw_area(
    aspect: Option<f32>,
    appearance: &str,
    [light, dark]: [&AreaColours; 2],
    seed: Option<AreaSeed>,
    drawn: AreaDrawn,
) {
    #[cfg(target_os = "macos")]
    {
        let (sans, mono) = subtake_ui::medium_faces();
        let seed = seed.unwrap_or_default();
        let colours = [*light, *dark];
        let appearance = match appearance {
            "light" => 1,
            "dark" => 2,
            _ => 0,
        };
        unsafe {
            subtake_draw_area(
                aspect.map_or(0.0, f64::from),
                appearance,
                colours.as_ptr().cast(),
                sans.as_ptr(),
                sans.len() as isize,
                mono.as_ptr(),
                mono.len() as isize,
                seed.display,
                seed.rect[0],
                seed.rect[1],
                seed.rect[2],
                seed.rect[3],
                drawn,
            );
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (aspect, appearance, light, dark, seed);
        drawn(0, 0.0, 0.0, 0.0, 0.0);
    }
}
