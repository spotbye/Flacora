//! Semantic opacity tokens and app colors resolved from the active Astryx palette.
use super::super::Tone;
use rune::Color;
use rune_kit::Kit;

pub(super) const OPAQUE: f32 = 1.0;
pub(super) const STATUS_FALLBACK_TINT_ALPHA: f32 = 0.06;

#[derive(Clone, Copy)]
pub(super) struct OpacityTokens {
    pub border: f32,
    pub surface_hover: f32,
    pub surface_pressed: f32,
    pub neutral_button: f32,
    pub submit_hover: f32,
    pub contrast_pressed: f32,
    pub scrim: f32,
    pub shadow_near: f32,
    pub shadow_far: f32,
    pub shadow_rim: f32,
    pub danger_hover: f32,
    pub danger_pressed: f32,
    pub scroll_thumb: f32,
    pub toast_border: f32,
    pub drop_overlay: f32,
    pub drop_target: f32,
}

pub(super) fn opacity(dark: bool) -> OpacityTokens {
    OpacityTokens {
        border: if dark { 0.10 } else { 0.08 },
        surface_hover: 0.05,
        surface_pressed: 0.10,
        neutral_button: if dark { 0.10 } else { 0.06 },
        submit_hover: if dark { 0.10 } else { 0.12 },
        contrast_pressed: if dark { 0.18 } else { 0.22 },
        scrim: if dark { 0.8 } else { 0.5 },
        shadow_near: if dark { 0.25 } else { 0.05 },
        shadow_far: if dark { 0.40 } else { 0.10 },
        shadow_rim: 0.08,
        danger_hover: 0.10,
        danger_pressed: 0.18,
        scroll_thumb: 0.72,
        toast_border: 0.28,
        drop_overlay: 0.97,
        drop_target: 0.35,
    }
}

#[derive(Clone, Copy)]
pub struct AppColors {
    pub scroll_thumb: Color,
    pub scroll_thumb_hover: Color,
    pub drop_overlay_surface: Color,
    pub drop_target_active: Color,
    pub danger_hover: Color,
    pub danger_pressed: Color,
    toast_borders: [Color; 3],
    kind_tiles: [[Color; 2]; 4],
}

impl AppColors {
    /// Resolve only after JSON presets have supplied their semantic palette.
    pub(super) fn new(k: &Kit, dark: bool) -> Self {
        let alpha = opacity(dark);
        let hex = |value| rune::color::hex(value).expect("Astryx kind tile color");
        let kind_tiles = if dark {
            [
                [hex("#253c5a"), hex("#a1caff")],
                [hex("#28413c"), hex("#81c9bb")],
                [hex("#4a2f51"), hex("#eaacf8")],
                [hex("#453a1c"), hex("#e3c36c")],
            ]
        } else {
            [
                [hex("#b9d7ff"), hex("#003978")],
                [hex("#a9e2d6"), hex("#00463d")],
                [hex("#f7d5ff"), hex("#5c0e6c")],
                [hex("#fae19e"), hex("#4b3900")],
            ]
        };
        Self {
            scroll_thumb: k.c.icon_muted.alpha(alpha.scroll_thumb),
            scroll_thumb_hover: k.c.icon_base.alpha(alpha.scroll_thumb),
            drop_overlay_surface: k.c.bg_base.alpha(alpha.drop_overlay),
            drop_target_active: k.c.layer_04.alpha(alpha.drop_target),
            danger_hover: k.c.danger.fg.alpha(alpha.danger_hover),
            danger_pressed: k.c.danger.fg.alpha(alpha.danger_pressed),
            toast_borders: [k.c.success, k.c.warning, k.c.danger]
                .map(|colors| colors.border.alpha(alpha.toast_border)),
            kind_tiles,
        }
    }

    pub fn toast_border(&self, kind: Tone) -> Color {
        self.toast_borders[match kind {
            Tone::Success => 0,
            Tone::Warning => 1,
            Tone::Error => 2,
        }]
    }

    /// Background and foreground colors for extension category tiles (0..4).
    pub fn kind_tile(&self, index: usize) -> [Color; 2] {
        self.kind_tiles[index.min(self.kind_tiles.len() - 1)]
    }
}
