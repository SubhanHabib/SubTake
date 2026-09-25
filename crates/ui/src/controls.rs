//! One file per control. The crate root re-exports everything here, so call
//! sites write `subtake_ui::button(..)` and never name the file.

mod button;
mod dropdown;
mod field_row;
mod glide;
mod input;
mod menu;
mod panel;
mod segmented_control;
mod slider;
mod status;
mod switch;
mod tile;
mod timecode;
mod tooltip;
mod zoom;

pub use button::{Button, button, focus_ring, hairline, icon_button, pressable, tool_button};
pub use dropdown::Dropdown;
pub use field_row::{field_row, group_card, setting_card, tile_grid};
pub use glide::{Glide, glide, glide_mark};
pub use input::{TextInput, init};
pub use menu::{command_row, menu_list, menu_row, menu_separator, menu_surface};
pub use panel::{
    Surface, caps_label, content_panel, divider, panel, panel_header, panel_variant, pod, pod_small,
};
pub use segmented_control::segmented_control;
pub use slider::Slider;
pub use status::{composer_footer, context_chip, progress_bar, status_chip, status_dot};
pub use switch::{switch, toggle};
pub use tile::{choice_tile, empty_state, media_tile, swatch};
pub use timecode::{TimecodeField, format_timecode};
pub use tooltip::{tooltip, tooltip_detail};
pub use zoom::{ZoomControl, zoom_control};
