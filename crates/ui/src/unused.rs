//! Primitives the handoff specifies and this build has nowhere to put yet.
//!
//! The redesign describes a complete kit. Ten of its pieces have no call site
//! in SubTake today: there is no search field, no numeric stepper, no
//! checkbox, no toast, no modal dialog, and the one list the app draws is a
//! grid of tiles rather than a column of rows. Rather than let those ten sit
//! unbuilt until somebody re-reads a spec that has already proved easy to
//! misread, they are built here, to the numbers the cards give, and parked.
//!
//! Nothing outside this module calls them, but for `key_cap` (a text
//! field's trailing cap, and the Add popup's shortcuts) and
//! `menu_section_header` (the Add popup's groups), which the Add popup
//! (`src/ui/add_popup.rs`) brought into use while it is compared with the
//! Add panel; they move out of here if the popup stays. `unused` is public so the
//! compiler does not warn about that, and so a gallery can render them to
//! check they still look right.
//!
//! Their measurements are module-local consts rather than `Theme` tokens, on
//! purpose: a token in `crates/theme` is part of the interface's vocabulary,
//! and these are not spoken yet. Promoting a primitive means moving its
//! numbers into `metrics.rs` along with it.

mod choice;
mod dialog;
mod field;
mod key_cap;
mod list_row;
mod menu_extras;
mod toast;

pub use choice::{checkbox, radio};
pub use dialog::{dialog, dialog_actions, scrim};
pub use field::{field, stepper};
pub use key_cap::key_cap;
pub use list_row::{RowState, list_row};
pub use menu_extras::{destructive_menu_row, menu_section_header, shortcut_hint, submenu_chevron};
pub use toast::toast;
