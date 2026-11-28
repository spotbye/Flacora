//! Astryx design components for Rune. Application state and business callbacks live in ui/.
mod components;
mod feedback;
pub mod icons;
mod preset;
pub mod theme;
mod widgets;
pub use components::{
    Button, Collapsible, Divider, IconButton, Menu, MenuItem, SegmentedControl, Sidebar,
    SidebarRow, Spinner, TextInput, ToastRegion,
};
pub use feedback::{Toast, progress, switch, theme_option, wrapping_picker};
pub use preset::ThemePreset;
pub use rune_kit::{ButtonVariant, ControlSize, IconButtonVariant, Placement, ScrollBar};
pub use theme::Kit;
pub use widgets::*;

#[derive(Clone, Copy)]
pub enum Tone {
    Success,
    Warning,
    Error,
}

pub fn root(base: rune::El, overlays: Vec<rune::El>) -> rune::El {
    rune_kit::root(base, overlays)
}

#[cfg(test)]
mod tests;
