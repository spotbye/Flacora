//! Shared Astryx surfaces, controls and composition patterns.
use super::icons;
use super::theme::{CARD_PADDING, FLACORA, Kit, PAGE_CONTENT_MAX_WIDTH, PAGE_INSET};
use super::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, SegmentedControl};
use rune::{
    Color, El, Icon, Image, LiveRegion, ScrollSpec, SurfaceFit, TextEdit, Tx, col, el, icon, row,
    text,
};
use rune_kit::{Badge, BadgeVariant, ScrollBar, ScrollBarStyle, StateColors};

pub fn page(key: &'static str) -> El {
    col()
        .key(key)
        .w_full()
        .max_w(PAGE_CONTENT_MAX_WIDTH + 2.0 * PAGE_INSET)
        .grow(1.0)
        .min_h(0.0)
        .padding(PAGE_INSET)
        .gap(FLACORA.spacing.section)
}

/// Page title with a one-line description and actions on the right.
pub fn page_header(k: &Kit, title: &str, subtitle: &str, actions: Vec<El>) -> El {
    page_header_with_subtitle(
        k,
        title,
        text(subtitle.to_owned())
            .font(k.ty.body)
            .color(k.c.text_muted)
            .wrap(),
        actions,
    )
}

pub fn page_header_with_subtitle(k: &Kit, title: &str, subtitle: El, actions: Vec<El>) -> El {
    let mut head = row()
        .key("page-head")
        .w_full()
        .gap(FLACORA.spacing.group)
        .center_y()
        .flex_wrap()
        .child(
            col()
                .gap(FLACORA.spacing.small)
                .grow(1.0)
                .shrink()
                .min_w(FLACORA.page.header_min_width)
                .child(
                    text(title.to_owned())
                        .role("heading")
                        .font(k.heading)
                        .color(k.c.text_strong),
                )
                .child(subtitle),
        );
    for action in actions {
        head = head.child(action);
    }
    head
}

/// A heading above a group of rows, with an optional count.
pub fn section(k: &Kit, title: &str, count: Option<usize>) -> El {
    let mut head = row().gap(FLACORA.spacing.inline).center_y().child(
        text(title.to_owned())
            .role("heading")
            .font(k.subheading)
            .color(k.c.text_strong),
    );
    if let Some(count) = count {
        head = head.child(badge(k, count.to_string()));
    }
    head
}

pub fn card(k: &Kit, key: impl Into<String>, body: El) -> El {
    col()
        .key(key)
        .role("group")
        .w_full()
        .min_w(0.0)
        .padding(CARD_PADDING)
        .radius(k.card_radius)
        .fill(k.c.surface_raised_base)
        .border(FLACORA.border.normal, k.c.border_base)
        .child(body)
}

pub fn badge(k: &Kit, label: impl Into<String>) -> El {
    Badge::new(label)
        .variant(BadgeVariant::Subtle)
        .build(k)
        .px(FLACORA.badge.padding_x)
        .h(FLACORA.badge.height)
        .radius(FLACORA.badge.radius)
}

pub fn badge_with_icon(k: &Kit, label: &str, leading: El) -> El {
    row()
        .center()
        .gap(FLACORA.spacing.small)
        .pr(FLACORA.spacing.inline)
        .child(leading.a11y_hidden())
        .child(badge(k, label).px(0.0).fill(Color::TRANSPARENT))
}

pub fn segmented(k: &Kit, key: &str, control: SegmentedControl) -> El {
    control.build(k, key)
}

pub fn filter_group(key: &str, label: &str) -> El {
    row()
        .key(key.to_owned())
        .role("radiogroup")
        .label(label.to_owned())
        .focus_group()
        .gap(FLACORA.spacing.small)
        .center_y()
        .flex_wrap()
}

pub fn filter_button<S: 'static>(
    k: &Kit,
    key: String,
    label: String,
    selected: bool,
    leading: Option<El>,
    select: impl Fn(&mut S, &mut Tx) + 'static,
) -> El {
    let mut button = Button::new(label.clone())
        .variant(if selected {
            ButtonVariant::Submit
        } else {
            ButtonVariant::Neutral
        })
        .control_size(ControlSize::Small)
        .on_press_tx(move |s, tx| {
            if !selected {
                select(s, tx);
            }
        });
    if let Some(leading) = leading {
        button = button.label_with(move |font, ink| {
            row()
                .gap(FLACORA.spacing.label)
                .center_y()
                .child(leading.a11y_hidden())
                .child(text(label).font(font).color(ink))
        });
    }
    button
        .compact()
        .build(k)
        .key(key)
        .role("radio")
        .checked(selected)
        .focus_selected(selected)
}

pub fn error_banner(k: &Kit, key: String, title: &str, detail: El, action: Option<El>) -> El {
    row()
        .key(key)
        .role("alert")
        .live_region(LiveRegion::Assertive)
        .w_full()
        .min_w(0.0)
        .gap(FLACORA.spacing.group)
        .padding(FLACORA.spacing.group)
        .items_start()
        .radius(k.radius.lg)
        .fill(k.c.danger.bg)
        .child(
            icon(&icons::CIRCLE_X, FLACORA.icons.large)
                .color(k.c.danger.icon)
                .a11y_hidden(),
        )
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .gap(FLACORA.spacing.text)
                .child(
                    text(title.to_owned())
                        .font(k.alert_heading)
                        .color(k.c.danger.fg)
                        .wrap(),
                )
                .child(detail.font(k.ty.body).color(k.c.danger.fg).wrap()),
        )
        .children(action)
}

pub fn confirmation_dialog<S: 'static>(
    k: &Kit,
    key: &str,
    title: &str,
    detail: String,
    trigger: &str,
    [keep, confirm]: [El; 2],
    dismiss: impl Fn(&mut S) + 'static,
) -> El {
    let metrics = k.metrics.dialog.expect("Astryx dialog metrics");
    let dialog = col()
        .key(format!("{key}-dialog"))
        .role("alertdialog")
        .label(title.to_owned())
        .description(detail.clone())
        .w_full()
        .max_w(FLACORA.overlay.dialog_max_width)
        .padding(metrics.padding)
        .gap(FLACORA.spacing.section)
        .radius(metrics.radius)
        .fill(k.c.surface_float)
        .border(FLACORA.border.normal, k.c.border_base)
        .modal(true, dismiss)
        .modal_click_outside(false)
        .focus_return(trigger.to_owned())
        .child(
            col()
                .gap(FLACORA.spacing.small)
                .child(
                    text(title.to_owned())
                        .role("heading")
                        .font(metrics.title.unwrap_or(k.subheading))
                        .color(k.c.text_strong),
                )
                .child(
                    text(detail)
                        .key(format!("{key}-counts"))
                        .font(k.ty.body)
                        .color(k.c.text_muted)
                        .wrap(),
                ),
        )
        .child(
            row()
                .gap(FLACORA.spacing.inline)
                .justify_end()
                .child(keep.autofocus().default_button())
                .child(confirm),
        );
    el().key(format!("{key}-overlay"))
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(k.c.scrim)
        .padding(FLACORA.overlay.margin)
        .center()
        .child(dialog)
}

pub fn scroll_bar_style(k: &Kit) -> ScrollBarStyle {
    ScrollBarStyle::from_kit(k)
        .thumb(k.colors.scroll_thumb)
        .thumb_hover(k.colors.scroll_thumb_hover)
}

pub fn page_scroll(k: &Kit, body: El) -> El {
    let bar = scroll_bar_style(k);
    body.scroll(ScrollSpec {
        stick_bottom: false,
    })
    .scroll_anchor()
    .grow(1.0)
    .min_h(0.0)
    // Place the thumb in the page's right gutter.
    .pr(bar.strip())
    .mr(-bar.strip())
    .scroll_bar_styled(bar)
}

pub fn status_badge(k: &Kit, glyph: &'static Icon, label: &str, status: Option<StateColors>) -> El {
    let (bg, ink) = status.map_or((k.c.layer_03, k.c.text_muted), |c| (c.bg, c.fg));
    row()
        .role("status")
        .label(label.to_owned())
        .h(FLACORA.badge.height)
        .px(FLACORA.badge.padding_x)
        .gap(FLACORA.spacing.small)
        .center()
        .radius(FLACORA.badge.status_radius)
        .fill(bg)
        .child(icon(glyph, FLACORA.icons.badge).color(ink).a11y_hidden())
        .child(
            text(label.to_owned())
                .font(k.ty.small_medium)
                .color(ink)
                .a11y_hidden(),
        )
}

/// Cover art with a music icon as its placeholder.
pub fn cover(k: &Kit, image: Option<&Image>, size: f32) -> El {
    let base = el()
        .size(size, size)
        .radius(k.radius.md)
        .fill(k.c.layer_02)
        .a11y_hidden();
    match image.filter(|i| i.error().is_none()) {
        Some(image) => base.pixels_fit(image.pixels(), SurfaceFit::Fill),
        None => base.center().child(
            icon(&icons::MUSIC, size * FLACORA.artwork.placeholder_ratio).color(k.c.icon_muted),
        ),
    }
}

pub fn icon_button<S: 'static>(
    k: &Kit,
    key: &str,
    glyph: &'static Icon,
    label: &str,
    on_press: impl Fn(&mut S) + 'static,
) -> El {
    IconButton::new(glyph)
        .variant(IconButtonVariant::Ghost)
        .control_size(ControlSize::Large)
        .label(label)
        .help(label)
        .on_press(on_press)
        .build(k)
        .key(key.to_owned())
}

pub fn clear_input_button<S: 'static>(
    k: &Kit,
    input_key: &'static str,
    label: &'static str,
    edit: impl Fn(&mut S) -> &mut TextEdit + 'static,
) -> El {
    IconButton::new(&icons::X)
        .variant(IconButtonVariant::Ghost)
        .control_size(ControlSize::Regular)
        .label(label)
        .help(label)
        .on_press_tx(move |s: &mut S, tx| {
            *edit(s) = TextEdit::default();
            tx.focus_when_shown(input_key, false);
        })
        .build(k)
}

pub fn danger_icon_button<S: 'static>(
    k: &Kit,
    key: &str,
    glyph: &'static Icon,
    label: &str,
    on_press: impl Fn(&mut S) + 'static,
) -> El {
    IconButton::new(glyph)
        .variant(IconButtonVariant::Ghost)
        .control_size(ControlSize::Large)
        .danger()
        .label(label)
        .help(label)
        .on_press(on_press)
        .build(k)
        .key(key.to_owned())
}

pub fn button<S: 'static>(
    k: &Kit,
    key: &str,
    label: &str,
    glyph: Option<&'static Icon>,
    variant: ButtonVariant,
    on_press: impl Fn(&mut S) + 'static,
) -> El {
    let mut b = Button::new(label)
        .variant(variant)
        .control_size(ControlSize::Regular)
        .on_press(on_press);
    if let Some(glyph) = glyph {
        b = b.icon(glyph);
    }
    b.action().build(k).key(key.to_owned())
}

/// A centred illustration, message and optional action, for empty pages.
pub fn empty_state(
    k: &Kit,
    key: &str,
    glyph: &'static Icon,
    title: &str,
    body: &str,
    action: Option<El>,
) -> El {
    col()
        .key(key.to_owned())
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(FLACORA.spacing.inline)
        .padding(FLACORA.spacing.inset)
        .center()
        .child(
            icon(glyph, FLACORA.page.empty_icon)
                .color(k.c.icon_base)
                .a11y_hidden(),
        )
        .child(
            text(title.to_owned())
                .role("heading")
                .level(3)
                .font(k.subheading)
                .color(k.c.text_strong)
                .wrap()
                .align(rune::Align::Center),
        )
        .child(
            text(body.to_owned())
                .font(k.ty.small)
                .color(k.c.text_muted)
                .max_w(FLACORA.page.empty_text_width)
                .wrap()
                .align(rune::Align::Center),
        )
        .children(action)
}

pub fn inline_link(k: &Kit, key: String, label: String) -> El {
    let metrics = k
        .inline_controls
        .metrics
        .button
        .expect("Astryx inline control metrics")
        .regular;
    row()
        .key(key)
        .role("link")
        .label(label.clone())
        .help(label)
        .focusable(true)
        .h(metrics.height)
        .px(metrics.padding_x)
        .center_y()
        .radius(metrics.radius)
        .cursor(rune::Cursor::Pointer)
        .hover(k.c.hover)
        .focus_ring(
            super::theme::FOCUS_RING_WIDTH,
            super::theme::FOCUS_RING_OFFSET,
            k.c.border_focus,
        )
}
