//! Astryx controls built on Rune core where the required Kit behavior differs.
use super::{
    Kit, Tone, icons,
    theme::{self, FLACORA, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH},
};
use rune::{Color, Cursor, El, LoopSpec, Transition, Tx, col, el, icon, row, text};

pub fn progress(k: &Kit, key: String, label: String, value: Option<f32>) -> El {
    let metrics = k.metrics.progress.expect("Astryx progress metrics");
    const SWEEP_WIDTH: f32 = FLACORA.queue.progress_sweep_fraction;

    let mut progress = col()
        .key(key)
        .role("progressbar")
        .label(label.clone())
        .w_full()
        .gap(FLACORA.spacing.small);
    let mut header = row()
        .w_full()
        .gap(FLACORA.spacing.inline)
        .center_y()
        .justify_between()
        .child(
            text(label)
                .font(k.ty.body)
                .color(k.c.text_base)
                .min_w(0.0)
                .shrink()
                .truncate(),
        );
    let fill = el()
        .h_full()
        .radius(metrics.fill_radius)
        .fill(k.c.bg_accent)
        .transition(Transition::none());
    let fill = match value.filter(|value| value.is_finite()) {
        Some(value) => {
            let value = value.clamp(0.0, 1.0);
            let percent = format!("{}%", (value * 100.0).round());
            progress = progress
                .range_value((f64::from(value) * 1e4).round() / 1e4, 0.0, 1.0)
                .a11y_value(percent.clone());
            header = header.child(
                text(percent)
                    .key("percentage")
                    .font(k.ty.small)
                    .color(k.c.text_muted)
                    .transition(Transition::none()),
            );
            fill.key("fill").w_frac(value)
        }
        None => fill
            .key("moving-fill")
            .w_frac(SWEEP_WIDTH)
            .looping(LoopSpec::slide(
                FLACORA.motion.progress_period,
                [-1.0, 0.0],
                [1.0 / SWEEP_WIDTH, 0.0],
            )),
    };
    let radius = metrics.height / 2.0;
    let track = row()
        .key("track")
        .w_full()
        .h(metrics.height)
        .radius(radius)
        .clip_rounded(radius)
        .border(metrics.border_width, k.c.border_strong)
        .fill(k.c.border_strong)
        .child(fill);
    progress
        .child(header.a11y_hidden())
        .child(track.a11y_hidden())
}

pub fn switch<S: 'static>(
    k: &Kit,
    key: &str,
    label: &str,
    on: bool,
    toggle: impl Fn(&mut S) + 'static,
) -> El {
    let profile = if on { &**k } else { &k.switch_off };
    let metrics = profile
        .metrics
        .switch
        .expect("Astryx switch metrics")
        .regular;
    let track = el()
        .key("track")
        .size(metrics.track_width, metrics.track_height)
        .radius(metrics.track_height / 2.0)
        .fill(if on { k.c.bg_accent } else { k.c.border_strong })
        .child(
            el().key("thumb")
                .size(metrics.knob_width, metrics.knob_height)
                .radius(metrics.knob_width.min(metrics.knob_height) / 2.0)
                .absolute(
                    if on {
                        metrics.track_width - metrics.knob_width - metrics.inset
                    } else {
                        metrics.inset
                    },
                    metrics.inset,
                )
                .fill(if on { k.c.knob_on } else { k.c.knob }),
        )
        .a11y_hidden();
    row()
        .key(key.to_owned())
        .role("switch")
        .label(label.to_owned())
        .checked(on)
        .focusable(true)
        .focus_ring(FOCUS_RING_WIDTH, FOCUS_RING_OFFSET, k.c.border_focus)
        .cursor(Cursor::Pointer)
        .radius(k.radius.xs)
        .hover(k.c.hover)
        .press(k.c.pressed)
        .min_h(FLACORA.settings.switch_min_height)
        .center()
        .on_press(toggle)
        .child(track)
}

pub fn wrapping_picker<S: 'static>(
    k: &Kit,
    key: &str,
    group_label: &str,
    labels: &[String],
    current: usize,
    pick: impl Fn(&mut S, usize) + Clone + 'static,
) -> El {
    let seg_k = &k.segmented;
    let metrics = k
        .default_picker
        .metrics
        .segmented
        .expect("Astryx default picker metrics");
    let count = labels.len();
    let mut group = row()
        .key(key.to_owned())
        .role("radiogroup")
        .label(group_label.to_owned())
        .focus_group()
        .gap(metrics.gap)
        .shrink()
        .min_w(0.0)
        .flex_wrap();
    for (i, label) in labels.iter().enumerate() {
        let selected = i == current;
        let pick = pick.clone();
        let pill = row()
            .key(format!("segment{i}"))
            .role("radio")
            .label(label.clone())
            .checked(selected)
            .position_in_set(i + 1, count)
            .focusable(true)
            .focus_selected(selected)
            .focus_ring(FOCUS_RING_WIDTH, FOCUS_RING_OFFSET, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .shrink()
            .min_w(0.0)
            .h(metrics.height - 2.0 * metrics.inset)
            .px(metrics.padding_x)
            .center()
            .radius(k.radius.xs)
            .fill(if selected {
                k.c.surface_raised_base
            } else {
                Color::TRANSPARENT
            })
            .hover(if selected {
                k.c.surface_raised_base
            } else {
                k.c.hover
            })
            .on_press(move |s: &mut S| pick(s, i))
            .child(
                text(label.clone())
                    .font(k.ty.ui_medium)
                    .color(if selected {
                        k.c.text_strong
                    } else {
                        seg_k.c.text_muted
                    })
                    .min_w(0.0)
                    .shrink()
                    .truncate()
                    .help_when_truncated(label.clone())
                    .a11y_hidden(),
            );
        group = group.child(pill);
    }
    row()
        .w(FLACORA.settings.default_picker_max_width)
        .shrink()
        .min_w(0.0)
        .justify_end()
        .child(
            row()
                .key(format!("{key}-track"))
                .shrink()
                .min_w(0.0)
                .padding(metrics.inset)
                .radius(metrics.radius)
                .fill(k.c.layer_01)
                .child(group),
        )
}

pub fn theme_option<S: 'static>(
    k: &Kit,
    key: &str,
    preset: super::ThemePreset,
    dark: bool,
    selected: bool,
    select: impl Fn(&mut S) + 'static,
) -> El {
    let metrics = k
        .inline_controls
        .metrics
        .button
        .expect("Astryx inline control metrics")
        .regular;
    let preview = theme::kit(preset, dark);
    let swatch = el()
        .size(FLACORA.swatch.outer, FLACORA.swatch.outer)
        .radius(FLACORA.swatch.outer_radius)
        .fill(preview.c.page)
        .border(FLACORA.border.normal, preview.c.border_strong)
        .center()
        .child(
            el().size(FLACORA.swatch.inner, FLACORA.swatch.inner)
                .radius(FLACORA.swatch.inner_radius)
                .fill(preview.c.bg_accent),
        );
    let option = row()
        .key(key.to_owned())
        .role("radio")
        .label(preset.label())
        .checked(selected)
        .position_in_set(preset as usize + 1, super::ThemePreset::ALL.len())
        .focusable(true)
        .focus_selected(selected)
        .focus_ring(FOCUS_RING_WIDTH, FOCUS_RING_OFFSET, k.c.border_focus)
        .cursor(Cursor::Pointer)
        .h(metrics.height)
        .px(metrics.padding_x)
        .gap(FLACORA.spacing.label)
        .center_y()
        .radius(metrics.radius)
        .fill(if selected { k.c.layer_04 } else { k.c.bg_base })
        .border(
            FLACORA.border.normal,
            if selected {
                k.c.border_focus
            } else {
                k.c.border_base
            },
        )
        .on_press(select)
        .child(swatch.a11y_hidden())
        .child(
            text(preset.label())
                .font(k.ty.small_medium)
                .color(k.c.text_strong)
                .a11y_hidden(),
        );
    k.interactive(option)
}

/// A semantic status toast. Astryx chooses its palette, icon, radius and border.
pub struct Toast {
    inner: rune_kit::Toast,
    tone: Tone,
}
impl Toast {
    pub fn new(k: &Kit, key: &str, tone: Tone, message: &str) -> Self {
        let (title, glyph, colors) = match tone {
            Tone::Success => ("Success", &icons::CIRCLE_CHECK, k.c.success),
            Tone::Warning => ("Warning", &icons::TRIANGLE_ALERT, k.c.warning),
            Tone::Error => ("Error", &icons::CIRCLE_X, k.c.danger),
        };
        Self {
            inner: rune_kit::Toast::new(key, title)
                .description(message)
                .lead(icon(glyph, FLACORA.icons.standard).color(colors.icon)),
            tone,
        }
    }
    pub fn on_close<S: 'static>(mut self, close: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_close(close);
        self
    }
    pub fn action<S: 'static>(
        mut self,
        k: &Kit,
        label: &str,
        f: impl Fn(&mut S, &mut Tx) + 'static,
    ) -> Self {
        self.inner = self.inner.action(k.toast(self.tone), label, true, f);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner
            .build(k.toast(self.tone))
            .w(FLACORA.overlay.toast_width)
            .border(FLACORA.border.normal, k.colors.toast_border(self.tone))
            .radius(k.card_radius)
            .transition(Transition::opacity())
    }
}
