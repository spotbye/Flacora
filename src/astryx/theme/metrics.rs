//! Astryx component geometry, preserving Flacora's existing control sizes.
//! App-only badges, spinners, artwork and layout live in `flacora_metrics`.
use rune::{Color, Family, Shadow, TextStyle};
use rune_kit::{
    ButtonMetrics, DialogMetrics, FieldMetrics, IconButtonMetrics, Kit, MenuMetrics,
    PopupRowMetrics, ProgressMetrics, Radius, SegmentedMetrics, SwitchMetrics, Type,
};

pub(super) const CONTROL_HEIGHT: f32 = 32.0;
pub(super) const COMPACT_HEIGHT: f32 = 28.0;
pub(super) const TOOLBAR_HEIGHT: f32 = 36.0;

/// Resolve metrics after the preset's radius, typography and palette are installed.
pub(super) fn apply(k: &mut Kit, dialog_radius: f32, dialog_title: TextStyle) {
    k.metrics.button = Some(ButtonMetrics::kit(k));
    k.metrics.icon_button = Some(IconButtonMetrics::kit(k));
    k.metrics.field = Some(FieldMetrics {
        height: CONTROL_HEIGHT,
        large_height: TOOLBAR_HEIGHT,
        ..FieldMetrics::kit(k)
    });
    k.metrics.segmented = Some(SegmentedMetrics::kit(k));
    k.metrics.disclosure_header_height = Some(CONTROL_HEIGHT);
    k.metrics.divider_thickness = Some(1.0);
    k.metrics.popup_row = Some(PopupRowMetrics::kit(k));
    k.metrics.menu = Some(MenuMetrics::kit(k));
    k.metrics.dialog = Some(DialogMetrics {
        radius: dialog_radius,
        padding: 16.0,
        title: Some(dialog_title),
    });
    k.metrics.switch = Some(rune_kit::BySize {
        regular: SwitchMetrics {
            track_width: 40.0,
            track_height: 24.0,
            knob_width: 20.0,
            knob_height: 20.0,
            inset: 2.0,
        },
        ..SwitchMetrics::NS_SWITCH
    });
    // The download bar has no rim and shares its 4 px corners with the track.
    k.metrics.progress = Some(ProgressMetrics {
        height: 8.0,
        border_width: 0.0,
        fill_radius: 4.0,
    });
}

/// The off state has a smaller circular thumb, unlike the stock switch's pill.
pub(super) fn switch_off(k: &Kit) -> Box<Kit> {
    let mut profile = Box::new(k.clone());
    let shape = &mut profile
        .metrics
        .switch
        .as_mut()
        .expect("Astryx switch metrics")
        .regular;
    shape.knob_width = 16.0;
    shape.knob_height = 16.0;
    shape.inset = 4.0;
    profile
}

/// Wrapping extension choices retain their 28 px pills inside a 4 px groove.
pub(super) fn default_picker(k: &Kit) -> Box<Kit> {
    let mut profile = Box::new(k.clone());
    profile.metrics.segmented = Some(SegmentedMetrics {
        height: 36.0,
        padding_x: 10.0,
        inset: 4.0,
        gap: 4.0,
        ..SegmentedMetrics::kit(k)
    });
    profile
}

/// Shared frame for theme choices and the sidebar's inline links.
pub(super) fn inline_controls(k: &Kit) -> Box<Kit> {
    let mut profile = buttons(k, 30.0, false);
    profile
        .metrics
        .button
        .as_mut()
        .expect("Astryx button metrics")
        .regular
        .padding_x = 8.0;
    profile
}

pub(super) fn buttons(k: &Kit, height: f32, compact_small: bool) -> Box<Kit> {
    let mut profile = Box::new(k.clone());
    let sizes = profile
        .metrics
        .button
        .as_mut()
        .expect("Astryx button metrics");
    sizes.regular.height = height;
    if compact_small {
        sizes.small.height = height;
    }
    profile
}

/// Neutral geometry is defined here before a preset overrides its type and radii.
pub(super) fn neutral_geometry(k: &mut Kit, dark: bool) {
    let alpha = super::tokens::opacity(dark);
    k.elevation.button_neutral.clear();
    k.elevation.button_contrast.clear();
    k.elevation.floating = vec![
        Shadow {
            y: 2.0,
            blur: 4.0,
            color: Color::BLACK.alpha(alpha.shadow_near),
            ..Shadow::default()
        },
        Shadow {
            y: 4.0,
            blur: 8.0,
            color: Color::BLACK.alpha(alpha.shadow_far),
            ..Shadow::default()
        },
    ];
    if dark {
        k.elevation.floating.push(Shadow {
            spread: 1.0,
            color: Color::WHITE.alpha(alpha.shadow_rim),
            inset: true,
            ..Shadow::default()
        });
    }
    input_elevation(k);
    k.radius = Radius {
        xs: 6.0,
        sm: 10.0,
        md: 10.0,
        lg: 10.0,
        xl: 10.0,
    };
    k.ty = Type {
        ui: TextStyle::new(14.0, 400.0).lh(20.0),
        ui_medium: TextStyle::new(14.0, 500.0).lh(20.0),
        ui_compact: TextStyle::new(14.0, 400.0).lh(20.0),
        body: TextStyle::new(14.0, 400.0).lh(20.0),
        body_medium: TextStyle::new(14.0, 600.0).lh(20.0),
        small: TextStyle::new(12.0, 400.0).lh(16.0),
        small_medium: TextStyle::new(12.0, 500.0).lh(16.0),
        text_small: TextStyle::new(12.0, 400.0).lh(20.0),
        text_small_medium: TextStyle::new(12.0, 500.0).lh(20.0),
        text_xsmall: TextStyle::new(12.0, 400.0).lh(16.0),
        text_xsmall_medium: TextStyle::new(12.0, 500.0).lh(16.0),
        mono: TextStyle::new(12.0, 400.0)
            .lh(18.0)
            .family(Family::SystemMono),
    };
}

pub(super) const CARD_RADIUS: f32 = 12.0;

pub(super) fn heading() -> TextStyle {
    TextStyle::new(24.0, 600.0).lh(32.0)
}

pub(super) fn subheading() -> TextStyle {
    TextStyle::new(17.0, 700.0).lh(24.0)
}

pub(super) fn alert_heading() -> TextStyle {
    TextStyle::new(14.0, 700.0).lh(20.0)
}

/// Share the same one-pixel input ring between Neutral and JSON presets.
pub(super) fn input_elevation(k: &mut Kit) {
    k.elevation = std::mem::take(&mut k.elevation).with_input(vec![Shadow {
        spread: 1.0,
        color: k.c.border_strong,
        inset: true,
        ..Shadow::default()
    }]);
}
