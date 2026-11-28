//! Theme assets from astryx-rune 8102215, based on Astryx a8001d84.

use super::super::ThemePreset;
use rune::{Color, Family, Theme};
use rune_kit::{Kit, Radius, StateColors, Type};

pub const FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/Figtree.ttf"),
    include_bytes!("../../../assets/themes/fonts/albertsans/AlbertSans[wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/dmsans/DMSans[opsz,wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/fraunces/Fraunces[SOFT,WONK,opsz,wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/fustat/Fustat[wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/montserrat/Montserrat[wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/outfit/Outfit[wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/playwriteustrad/PlaywriteUSTrad[wght].ttf"),
    include_bytes!("../../../assets/themes/fonts/poppins/Poppins-Regular.ttf"),
    include_bytes!("../../../assets/themes/fonts/poppins/Poppins-Medium.ttf"),
    include_bytes!("../../../assets/themes/fonts/poppins/Poppins-SemiBold.ttf"),
    include_bytes!("../../../assets/themes/fonts/poppins/Poppins-Bold.ttf"),
];

pub(super) fn load(preset: ThemePreset, dark: bool) -> Option<(Theme, Family, Family)> {
    let (raw, body, heading) = match preset {
        ThemePreset::Neutral => return None,
        ThemePreset::Butter => (
            include_str!("../../../assets/themes/butter.json"),
            "Outfit",
            "Outfit",
        ),
        ThemePreset::Chocolate => (
            include_str!("../../../assets/themes/chocolate.json"),
            "Albert Sans",
            "Fraunces",
        ),
        ThemePreset::Gothic => (
            include_str!("../../../assets/themes/gothic.json"),
            "Fustat",
            "Fustat",
        ),
        ThemePreset::Matcha => (
            include_str!("../../../assets/themes/matcha.json"),
            "DM Sans",
            "Playwrite US Trad",
        ),
        ThemePreset::Stone => (
            include_str!("../../../assets/themes/stone.json"),
            "Figtree",
            "Montserrat",
        ),
        ThemePreset::Y2k => (
            include_str!("../../../assets/themes/y2k.json"),
            "Poppins",
            "Poppins",
        ),
    };
    let tokens = Theme::from_json(raw, dark).expect("embedded Astryx theme");
    Some((tokens, Family::Named(body), Family::Named(heading)))
}

pub(super) fn set_body_family(ty: &mut Type, family: Family) {
    for style in [
        &mut ty.ui,
        &mut ty.ui_medium,
        &mut ty.ui_compact,
        &mut ty.body,
        &mut ty.body_medium,
        &mut ty.small,
        &mut ty.small_medium,
        &mut ty.text_small,
        &mut ty.text_small_medium,
        &mut ty.text_xsmall,
        &mut ty.text_xsmall_medium,
    ] {
        *style = style.family(family);
    }
}

pub(super) fn apply(tokens: &Theme, kit: &mut Kit, dark: bool) {
    let alpha = super::tokens::opacity(dark);
    let color = |name| tokens.color(name);
    let c = &mut kit.c;
    c.page = color("backgroundBody");
    c.bg_base = color("backgroundSurface");
    c.bg_deep = c.page;
    c.layer_01 = color("backgroundMuted");
    c.layer_02 = c.layer_01;
    c.layer_03 = c.layer_01;
    c.layer_04 = color("accentMuted");
    c.bg_inverse = color("backgroundInverted");
    c.bg_contrast = color("accent");
    c.bg_button_neutral = color("neutral");
    c.bg_icon_button_contrast = c.bg_contrast;
    c.bg_accent = c.bg_contrast;
    c.submit_fg = color("onAccent");
    c.submit_icon = c.submit_fg;
    c.text_base = color("textPrimary");
    c.text_strong = c.text_base;
    c.text_muted = color("textSecondary");
    c.text_weak = c.text_muted;
    c.text_faint = c.text_muted;
    c.text_accent = color("textAccent");
    c.text_inverse = c.submit_fg;
    c.text_contrast = c.submit_fg;
    c.text_invert_strong = c.submit_fg;
    c.icon_base = color("iconPrimary");
    c.icon_muted = color("iconSecondary");
    c.icon_faint = color("iconDisabled");
    c.icon_weaker = c.icon_muted;
    c.icon_accent = color("iconAccent");
    c.icon_inverse = c.submit_fg;
    c.icon_contrast = c.submit_fg;
    c.border_base = color("border");
    c.border_weak = c.border_base;
    c.border_strong = color("borderEmphasized");
    c.border_muted = c.border_strong;
    c.border_hover = c.border_strong;
    c.border_disabled = c.border_base;
    c.border_focus = c.bg_accent;
    c.border_active = c.border_focus;
    c.hover = color("overlayHover");
    c.pressed = color("overlayPressed");
    c.submit_hover = c.submit_fg.alpha(alpha.submit_hover);
    c.contrast_hover = c.submit_hover;
    c.contrast_pressed = c.submit_fg.alpha(alpha.contrast_pressed);
    c.scrim = color("overlay");
    c.surface_base = c.border_strong;
    c.surface_weak = c.layer_01;
    c.surface_hover = c.hover;
    c.surface_raised = color("backgroundCard");
    c.surface_raised_base = c.surface_raised;
    c.surface_raised_hover = c.hover;
    c.surface_float = color("backgroundPopover");
    c.surface_disabled = c.layer_01;
    c.std_border = c.border_base;
    c.std_icon = c.icon_muted;
    c.std_icon_invert = c.submit_icon;
    c.std_icon_strong = c.icon_base;
    c.std_text = c.text_base;
    c.knob = c.bg_base;
    c.knob_on = c.submit_fg;

    let status = |name: &str| {
        let muted = tokens.color(&format!("{}Muted", name.to_lowercase()));
        let fg = tokens.color(&format!("text{name}"));
        StateColors {
            bg: status_background(c.surface_raised, muted, fg),
            fg,
            border: tokens.color(&format!("border{name}")),
            icon: tokens.color(&format!("icon{name}")),
        }
    };
    c.info = status("Info");
    c.success = status("Success");
    c.warning = status("Warning");
    c.danger = status("Error");

    super::metrics::input_elevation(kit);
    let inner = tokens.num("radius.inner");
    let element = tokens.num("radius.element");
    kit.radius = Radius {
        xs: inner,
        sm: element,
        md: element,
        lg: element,
        xl: element,
    };
}

// Flacora uses the same status ink on plain surfaces and within status panels.
fn status_background(surface: Color, muted: Color, ink: Color) -> Color {
    let background = surface.mix(muted, muted.a).alpha(super::tokens::OPAQUE);
    let (bg, fg) = (background.luminance() + 0.05, ink.luminance() + 0.05);
    if bg.max(fg) / bg.min(fg) >= 4.5 {
        background
    } else {
        surface.mix(ink, super::tokens::STATUS_FALLBACK_TINT_ALPHA)
    }
}
