//! Astryx theme, typography, metrics and cached component profiles.

mod presets;
mod metrics;
mod flacora_metrics;
mod tokens;

use super::Tone;
use super::icons;
use super::ThemePreset;
use rune::{Color, TextStyle};
use rune_kit::{Kit as RuneKit, StateColors};
use std::{ops::Deref, sync::OnceLock};

pub use presets::FONTS;
pub use flacora_metrics::FLACORA;
pub const PAGE_INSET: f32 = FLACORA.page.inset;
pub const PAGE_CONTENT_MAX_WIDTH: f32 = FLACORA.page.content_max_width;
pub const CARD_PADDING: f32 = FLACORA.page.card_padding;
pub const FOCUS_RING_WIDTH: f32 = FLACORA.focus.width;
pub const FOCUS_RING_OFFSET: f32 = FLACORA.focus.offset;

pub struct Kit {
    base: RuneKit,
    pub colors: tokens::AppColors,
    pub heading: TextStyle,
    pub subheading: TextStyle,
    pub alert_heading: TextStyle,
    pub card_radius: f32,
    pub drop_hint: Color,
    /// Existing 32 px actions, 28 px compact actions, and 36 px catalog actions.
    pub(super) controls: Box<RuneKit>,
    pub(super) compact: Box<RuneKit>,
    pub(super) toolbar: Box<RuneKit>,
    pub(super) switch_off: Box<RuneKit>,
    pub(super) default_picker: Box<RuneKit>,
    pub(super) inline_controls: Box<RuneKit>,
    pub(super) segmented: Box<RuneKit>,
    pub(super) danger_button: Box<RuneKit>,
    pub(super) danger_icon: Box<RuneKit>,
    toasts: [Box<RuneKit>; 3],
}

impl Deref for Kit {
    type Target = RuneKit;

    fn deref(&self) -> &RuneKit {
        &self.base
    }
}

impl Kit {
    pub(super) fn toast(&self, kind: Tone) -> &RuneKit {
        &self.toasts[match kind {
            Tone::Success => 0,
            Tone::Warning => 1,
            Tone::Error => 2,
        }]
    }
}

pub fn kit(preset: ThemePreset, dark: bool) -> &'static Kit {
    static KITS: [[OnceLock<Kit>; 2]; ThemePreset::ALL.len()] =
        [const { [const { OnceLock::new() }; 2] }; ThemePreset::ALL.len()];
    // Gothic uses the same dark palette in both modes.
    let dark = dark || preset == ThemePreset::Gothic;
    KITS[preset as usize][usize::from(dark)].get_or_init(|| build(preset, dark))
}

fn build(preset: ThemePreset, dark: bool) -> Kit {
    let mut base = neutral(dark);
    let mut heading = metrics::heading();
    let mut subheading = metrics::subheading();
    let mut alert_heading = metrics::alert_heading();
    let mut card_radius = metrics::CARD_RADIUS;
    if let Some((tokens, body_family, heading_family)) = presets::load(preset, dark) {
        presets::apply(&tokens, &mut base, dark);
        presets::set_body_family(&mut base.ty, body_family);
        heading = heading.family(heading_family);
        subheading = subheading.family(heading_family);
        alert_heading = alert_heading.family(heading_family);
        card_radius = tokens.num("radius.container");
    }

    let colors = tokens::AppColors::new(&base, dark);
    metrics::apply(&mut base, card_radius, subheading);
    let controls = metrics::buttons(&base, metrics::CONTROL_HEIGHT, false);
    let compact = metrics::buttons(&base, metrics::COMPACT_HEIGHT, true);
    let toolbar = metrics::buttons(&base, metrics::TOOLBAR_HEIGHT, false);
    let switch_off = metrics::switch_off(&base);
    let default_picker = metrics::default_picker(&base);
    let inline_controls = metrics::inline_controls(&base);

    let drop_hint = if dark && preset == ThemePreset::Neutral {
        rune::color::hex("#c6c6c6").expect("Astryx color")
    } else {
        base.c.text_muted
    };
    // Cached profiles live on the heap to keep theme construction's stack bounded.
    let mut segmented = Box::new(base.clone());
    segmented.c.border_base = Color::TRANSPARENT;
    segmented.c.border_strong = Color::TRANSPARENT;
    segmented.c.text_muted = drop_hint;
    segmented.metrics.segmented.as_mut().expect("Astryx segmented metrics").gap = 4.0;

    let mut danger_button = controls.clone();
    danger_button.c.border_focus = base.c.danger.border;

    let mut danger_icon = danger_button.clone();
    danger_icon.c.text_base = base.c.danger.icon;
    danger_icon.c.hover = colors.danger_hover;
    danger_icon.c.pressed = colors.danger_pressed;

    let toasts = [base.c.success, base.c.warning, base.c.danger].map(|colors| {
        let mut k = base.clone();
        k.c.layer_01 = colors.bg;
        k.c.text_base = colors.fg;
        k.c.text_muted = colors.fg;
        k.c.icon_muted = colors.icon;
        k.c.border_focus = colors.fg;
        Box::new(k)
    });
    Kit { base, colors, heading, subheading, alert_heading, card_radius, drop_hint, controls, compact, toolbar, switch_off, default_picker, inline_controls, segmented, danger_button, danger_icon, toasts }
}

// Neutral follows Astryx at 82ff7bbf64867ab047db3235d24667d15a72f97d.
fn neutral(dark: bool) -> RuneKit {
    let alpha = tokens::opacity(dark);
    let color = |light, night| rune::color::hex(if dark { night } else { light }).expect("Astryx color");
    let body = color("#f1f1f1", "#1b1b1b");
    let surface = color("#ffffff", "#262626");
    let card = color("#ffffff", "#262626");
    let text = color("#000000", "#ffffff");
    let secondary = color("#474747", "#9e9e9e");
    let icon_secondary = color("#6a6a6a", "#9e9e9e");
    let accent = color("#1b1b1b", "#f1f1f1");
    let on_accent = color("#ffffff", "#111111");
    let selected = color("#d4d4d4", "#474747");
    let gray = color("#e2e2e2", "#303030");
    let border = text.alpha(alpha.border);
    let emphasized = color("#d4d4d4", "#525252");
    let hover = text.alpha(alpha.surface_hover);
    let pressed = text.alpha(alpha.surface_pressed);
    let submit_hover = (if dark { Color::BLACK } else { Color::WHITE }).alpha(alpha.submit_hover);

    let mut k = RuneKit::neutral(dark);
    let c = &mut k.c;
    c.page = body;
    c.bg_base = surface;
    c.bg_deep = body;
    c.layer_01 = gray;
    c.layer_02 = gray;
    c.layer_03 = gray;
    c.layer_04 = selected;
    c.bg_inverse = text;
    c.bg_contrast = accent;
    c.bg_button_neutral = text.alpha(alpha.neutral_button);
    c.bg_icon_button_contrast = accent;
    c.bg_accent = accent;
    c.submit_highlight = Color::TRANSPARENT;
    c.submit_hover = submit_hover;
    c.submit_fg = on_accent;
    c.submit_icon = on_accent;
    c.text_base = text;
    c.text_strong = text;
    c.text_muted = secondary;
    c.text_weak = secondary;
    c.text_faint = secondary;
    c.text_accent = accent;
    c.text_inverse = on_accent;
    c.text_contrast = on_accent;
    c.text_invert_strong = on_accent;
    c.icon_base = text;
    c.icon_muted = icon_secondary;
    c.icon_faint = icon_secondary;
    c.icon_weaker = icon_secondary;
    c.icon_accent = accent;
    c.icon_inverse = on_accent;
    c.icon_contrast = on_accent;
    c.border_muted = emphasized;
    c.border_base = border;
    c.border_weak = border;
    c.border_strong = emphasized;
    c.border_hover = emphasized;
    c.border_disabled = border;
    c.border_focus = accent;
    c.border_active = accent;
    c.surface_base = emphasized;
    c.surface_weak = gray;
    c.surface_hover = hover;
    c.surface_raised = card;
    c.surface_raised_base = card;
    c.surface_raised_hover = hover;
    c.surface_float = card;
    c.surface_disabled = gray;
    c.std_border = border;
    c.std_icon = icon_secondary;
    c.std_icon_invert = on_accent;
    c.std_icon_strong = text;
    c.std_text = text;
    c.hover = hover;
    c.pressed = pressed;
    c.contrast_hover = c.submit_hover;
    c.contrast_pressed = (if dark { Color::BLACK } else { Color::WHITE }).alpha(alpha.contrast_pressed);
    c.scrim = Color::BLACK.alpha(alpha.scrim);
    c.knob = surface;
    c.knob_on = on_accent;

    c.info = status(color("#b9d7ff", "#253c5a"), color("#003978", "#a1caff"));
    c.success = status(color("#bce0bb", "#2b422b"), color("#00490b", "#90ca90"));
    c.warning = status(color("#fae19e", "#453a1c"), color("#4b3900", "#eec448"));
    c.danger = status(color("#ffc4be", "#5b2b28"), color("#76000c", "#ffaea7"));

    metrics::neutral_geometry(&mut k, dark);
    k.glyph.clear = &icons::X;
    k.glyph.close = &icons::X;
    k
}

fn status(bg: Color, fg: Color) -> StateColors {
    StateColors { bg, fg, border: fg, icon: fg }
}
