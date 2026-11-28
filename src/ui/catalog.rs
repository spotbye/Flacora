use super::theme::FLACORA;
use super::{theme::Kit, widgets::{self, card}};
use crate::{app::{App, Page}, catalog::{Item, Kind, ResultPage}, icons, services};
use rune::{Cursor, El, GrowthSpec, Icon, ScrollPos, SurfaceFit, Tx, col, el, icon, row, text};
use crate::astryx::{Button, ButtonVariant, ControlSize, Divider, IconButton, IconButtonVariant, Spinner, TextInput};
use std::{borrow::Cow, cell::Cell, collections::BTreeMap, rc::Rc};

fn provider_display_name(name: &str) -> &str {
    name.strip_suffix(" Web").unwrap_or(name)
}

pub fn page(s: &App, k: &'static Kit) -> El {
    let head = widgets::page_header(k, "Catalog", "Browse music services and choose tracks to send to Downloads.", Vec::new());
    let providers: Vec<_> = s.catalog_providers().collect();
    let body = if !s.loaded {
        row()
            .key("catalog-loading")
            .role("status")
            .gap(FLACORA.spacing.inline)
            .center_y()
            .child(Spinner::new().size(FLACORA.spinner.inline).build(k))
            .child(text("Loading Catalog…").font(k.ty.body).color(k.c.text_muted))
    } else if providers.is_empty() {
        widgets::empty_state(k, "catalog-empty", &icons::SEARCH, "Install a Catalog Extension",
            s.catalog.error.as_deref().unwrap_or("Install a catalog extension for your music service."),
            Some(widgets::button(k, "catalog-install", "Go to Extensions", Some(&icons::PUZZLE), ButtonVariant::Submit,
                |s: &mut App| s.page = Page::Extensions)))
    } else {
        let mut bar = widgets::filter_group("catalog-providers", "Catalog Provider");
        let current = s.chosen_catalog_provider().map(|p| p.manifest.id.as_str());
        let provider_count = providers.len();
        for (idx, provider) in providers.into_iter().enumerate() {
            let id = provider.manifest.id.clone();
            let selected = current == Some(&id);
            let key = format!("catalog-provider-{id}");
            let label = provider_display_name(&provider.manifest.name).to_owned();
            let logo = services::by_name(&label).and_then(|svc| widgets::provider_logo(s, svc, FLACORA.artwork.provider_inline));
            bar = bar.child(widgets::filter_button(k, key, label, selected, logo, move |s: &mut App, _| {
                s.catalog.select_provider(id.clone());
            }).position_in_set(idx + 1, provider_count));
        }
        let mut search = TextInput::new(&s.catalog.input)
            .placeholder("Search or Paste a Music Link…")
            .leading_icon(&icons::SEARCH).large()
            .on_edit(|s: &mut App| &mut s.catalog.input)
            .on_submit(|s: &mut App, _: &mut Tx| s.submit_catalog());
        if !s.catalog.input.text.is_empty() {
            search = search.trailing(
                widgets::clear_input_button(k, "catalog-input", "Clear Search", |s: &mut App| &mut s.catalog.input)
                    .key("catalog-clear-search"),
            );
        }
        let search = search.build(k).grow(1.0).min_w(0.0).key("catalog-input");
        let home_active = s.catalog.home_feed_active();
        let home = Button::new("Home Feed")
            .icon(&icons::LAYOUT_GRID)
            .control_size(ControlSize::Regular)
            .variant(if home_active { ButtonVariant::Submit } else { ButtonVariant::Outline })
            .expanded(home_active)
            .on_press(App::toggle_catalog_home)
            .toolbar().build(k).key("catalog-home");
        let can_submit = !s.catalog.input.text.trim().is_empty() && s.importing == 0;
        let submit = Button::new("Go").icon(&icons::ARROW_RIGHT)
            .variant(ButtonVariant::Submit).control_size(ControlSize::Regular)
            .disabled(!can_submit)
            .on_press(|s: &mut App| s.submit_catalog())
            .toolbar().build(k).key("catalog-submit");
        let actions = row().w_full().gap(FLACORA.spacing.inline).center_y().child(search)
            .child(submit)
            .child(home);
        let mut body = col().w_full().grow(1.0).min_h(0.0).gap(FLACORA.spacing.group)
            .child(card(k, "catalog-controls", col().gap(FLACORA.spacing.row).child(bar).child(actions)));
        if s.catalog.busy() || s.catalog.can_go_back() {
            let mut status = row().gap(FLACORA.spacing.inline).center_y();
            if s.catalog.can_go_back() {
                status = status.child(widgets::button(k, "catalog-back", "Back", None, ButtonVariant::Ghost, |s: &mut App| s.catalog.back()));
            }
            if s.catalog.busy() {
                status = status.child(Spinner::new().size(FLACORA.spinner.inline).build(k))
                    .child(text("Loading Catalog…").font(k.ty.small).color(k.c.text_muted))
                    .child(widgets::button(k, "catalog-cancel", "Cancel", None, ButtonVariant::Ghost, |s: &mut App| s.catalog.cancel()));
            }
            body = body.child(status);
        }
        if let Some(error) = &s.catalog.error {
            let missing_extension = error == crate::catalog::MISSING_CATALOG_EXTENSION;
            let title = if missing_extension { "Catalog Extension Required" } else { "Catalog Request Failed" };
            let action = if missing_extension {
                Some(widgets::button(
                    k,
                    "catalog-open-extensions",
                    "Go to Extensions",
                    Some(&icons::PUZZLE),
                    ButtonVariant::Neutral,
                    |s: &mut App| s.page = Page::Extensions,
                ))
            } else {
                s.catalog.can_retry().then(|| widgets::button(
                    k, "catalog-retry", "Try Again", Some(&icons::REFRESH_CW), ButtonVariant::Neutral,
                    App::retry_catalog,
                ))
            };
            body = body.child(widgets::error_banner(
                k, "catalog-error-banner".into(), title, text(error.clone()).key("catalog-error"), action,
            ));
        }
        if let Some(result) = &s.catalog.result {
            body = body.child(results(s, k, result));
        } else if !s.catalog.busy() && s.catalog.error.is_none() {
            body = body.child(widgets::empty_state(
                k,
                "catalog-idle",
                &icons::SEARCH,
                "Browse Music Catalogs",
                "Pick a provider above, search for a track or album, paste a music link, or open Home Feed.",
                None,
            ));
        }
        body
    };
    widgets::page("catalog-page").child(head).child(body)
}

fn results(s: &App, k: &'static Kit, result: &ResultPage) -> El {
    let tracks = result.items.iter().any(|item| item.kind == Kind::Track);
    let all_selected = tracks && result.items.iter().filter(|item| item.kind == Kind::Track).all(|item| s.catalog.selected.contains(&item.url));
    let mut header = row().gap(FLACORA.spacing.row).w_full().center_y().flex_wrap();
    if let Some(url) = result.artwork.as_deref() {
        header = header.child(artwork(s, k, "catalog-cover", Some(url), &icons::DISC, FLACORA.artwork.catalog_cover, false));
    }
    header = header.child(text(result.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate().grow(1.0).min_w(0.0));
    if tracks {
        let (label, glyph) = if all_selected { ("Clear Selection", &icons::X) } else { ("Select Loaded Tracks", &icons::CIRCLE_CHECK) };
        header = header.child(widgets::button(k, "catalog-select-all", label, Some(glyph), ButtonVariant::Outline,
            move |s: &mut App| {
                if all_selected {
                    s.catalog.selected.clear();
                } else if let Some(result) = &s.catalog.result {
                    for item in result.items.iter().filter(|item| item.kind == Kind::Track) {
                        s.catalog.selected.insert(item.url.clone());
                    }
                }
            }));
    }
    if !s.catalog.selected.is_empty() {
        if !all_selected {
            header = header.child(widgets::button(k, "catalog-clear-selection", "Clear", Some(&icons::X), ButtonVariant::Ghost,
                |s: &mut App| s.catalog.selected.clear()));
        }
        header = header.child(widgets::button(k, "catalog-use", &format!("To Downloads ({})", s.catalog.selected.len()),
            Some(&icons::ARROW_RIGHT), ButtonVariant::Submit, |s: &mut App| s.prepare_catalog_tracks()));
    }
    let home = s.catalog.showing_home();
    let mut list = col().w_full().gap(FLACORA.spacing.inline).pb(FLACORA.spacing.section)
        .child(if home && !result.items.is_empty() { feed(s, k, result) } else { result_rows(s, k, result) });
    if result.next.is_some() && !s.catalog.busy() {
        list = list.child(widgets::button(k, "catalog-more", "Load More", None, ButtonVariant::Outline,
            |s: &mut App| s.catalog_more()).self_center());
    }
    let mut content = col().key("catalog-result-page").w_full().grow(1.0).min_h(0.0).gap(FLACORA.spacing.label).child(header);
    if !result.note.is_empty() {
        content = content.child(text(result.note.clone()).font(k.ty.small).color(k.c.text_muted).wrap());
    }
    let key = if home { "catalog-feed" } else { "catalog-results" };
    content.child(widgets::page_scroll(k, col().key(key).child(list)))
}

fn result_rows(_s: &App, k: &'static Kit, result: &ResultPage) -> El {
    if result.items.is_empty() {
        return col().w_full().child(text("No items found.").font(k.ty.body).color(k.c.text_muted).py(FLACORA.spacing.section));
    }
    let count = result.items.len();
    col().key("catalog-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        count,
        FLACORA.catalog.estimated_row_height,
        |s: &App, i| {
            s.catalog
                .result
                .as_ref()
                .and_then(|page| page.items.get(i))
                .map(|item| Cow::Borrowed(item.url.as_str()))
                .unwrap_or_else(|| Cow::Owned(format!("catalog-row-{i}")))
        },
        move |s: &App, i| {
            let Some(item) = s.catalog.result.as_ref().and_then(|page| page.items.get(i)) else {
                return el();
            };
            let mut wrap = col().w_full().gap(FLACORA.spacing.small).pb(FLACORA.spacing.small);
            if i > 0 {
                wrap = wrap.child(Divider::horizontal().build(k));
            }
            wrap.child(item_row(s, k, i, item))
        },
    )
}

fn feed(s: &App, k: &Kit, result: &ResultPage) -> El {
    let shelves: Vec<(&str, Vec<&Item>)> = if result.sections.is_empty() {
        [Kind::Track, Kind::Album, Kind::Playlist, Kind::Artist].into_iter()
            .map(|kind| (kind.plural(), result.items.iter().filter(|item| item.kind == kind).collect()))
            .collect()
    } else {
        let items: BTreeMap<_, _> = result.items.iter().map(|item| (item.url.as_str(), item)).collect();
        result.sections.iter().map(|section| {
            let cards = section.urls.iter().filter_map(|url| items.get(url.as_str()).copied()).collect();
            (section.title.as_str(), cards)
        }).collect()
    };
    col().w_full().gap(FLACORA.spacing.shelf).children(shelves.into_iter().filter(|(_, items)| !items.is_empty()).enumerate()
        .map(|(index, (title, items))| feed_shelf(s, k, index, title, &items)))
}

fn feed_shelf(s: &App, k: &Kit, index: usize, title: &str, items: &[&Item]) -> El {
    let key = format!("catalog-shelf-{index}");
    let position = Rc::new(Cell::new(ScrollPos::default()));
    let watch = position.clone();
    let cards = row().gap(FLACORA.spacing.row).children(items.iter().enumerate().map(|(item_index, item)| {
        feed_card(s, k, format!("catalog-feed-item-{index}-{item_index}"), item)
    }));
    let shelf = col().key(key.clone()).w_full().min_w(0.0).scroll_x().scroll_edge_fade(FLACORA.catalog.edge_fade)
        .on_scroll_offset(move |at| watch.set(at))
        .child(cards);
    let mut head = row().w_full().center_y().gap(FLACORA.spacing.small)
        .child(widgets::section(k, title, None).grow(1.0).min_w(0.0));
    if items.len() > 1 {
        for (direction, glyph, label, name) in [
            (-1.0, &icons::CHEVRON_LEFT, "Scroll Left", "prev"),
            (1.0, &icons::CHEVRON_RIGHT, "Scroll Right", "next"),
        ] {
            let viewport = key.clone();
            let position = position.clone();
            head = head.child(IconButton::new(glyph).variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Large).label(label).help(label)
                .on_press_tx(move |_: &mut App, tx| {
                    let at = position.get();
                    let x = (at.offset[0] + direction * at.viewport[0] * FLACORA.catalog.scroll_fraction).clamp(0.0, at.max[0]);
                    tx.scroll_to(viewport.clone(), x, 0.0);
                })
                .build(k).key(format!("{key}-{name}")));
        }
    }
    col().w_full().min_w(0.0).gap(FLACORA.spacing.label).child(head).child(shelf)
}

fn feed_card(s: &App, k: &Kit, key: String, item: &Item) -> El {
    let selected = s.catalog.selected.contains(&item.url);
    let action = match (item.kind, selected) {
        (Kind::Track, true) => "Deselect",
        (Kind::Track, false) => "Select",
        _ => "Open",
    };
    let url = item.url.clone();
    let kind = item.kind;
    let mut title = row().w_full().gap(FLACORA.spacing.small).center_y()
        .child(text(item.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate().grow(1.0).min_w(0.0));
    if selected {
        title = title.child(icon(&icons::CIRCLE_CHECK, FLACORA.icons.inline).color(k.c.success.fg));
    }
    let mut card = col().key(key.clone()).w(FLACORA.catalog.tile_width).padding(FLACORA.spacing.inline).gap(FLACORA.spacing.small)
        .role("button").label(format!("{action} {}", item.title)).focusable(true)
        .focus_ring(super::theme::FOCUS_RING_WIDTH, super::theme::FOCUS_RING_OFFSET, k.c.border_focus).cursor(Cursor::Pointer)
        .radius(k.radius.md).fill(k.c.surface_raised_base).hover(k.c.hover)
        .border(FLACORA.border.normal, if selected { k.c.border_focus } else { k.c.border_base })
        .on_press(move |s: &mut App| {
            if kind == Kind::Track {
                toggle_track(s, &url);
            } else {
                s.open_catalog(url.clone(), true);
            }
        })
        .child(artwork(s, k, &format!("{key}-artwork"), item.artwork.as_deref(), item_icon(kind), FLACORA.artwork.catalog_tile, kind == Kind::Artist))
        .child(title)
        .child(text(if item.detail.is_empty() { item.kind.label().to_owned() } else { item.detail.clone() })
            .font(k.ty.small).color(k.c.text_muted).truncate());
    if kind == Kind::Track {
        card = card.checked(selected);
    }
    card
}

fn toggle_track(s: &mut App, url: &str) {
    if !s.catalog.selected.remove(url) {
        s.catalog.selected.insert(url.to_owned());
    }
}

fn item_icon(kind: Kind) -> &'static Icon {
    match kind {
        Kind::Track => &icons::MUSIC,
        Kind::Album => &icons::DISC,
        Kind::Playlist => &icons::LIST_MUSIC,
        Kind::Artist => &icons::MIC_VOCAL,
    }
}

fn item_row(s: &App, k: &Kit, index: usize, item: &Item) -> El {
    let url = item.url.clone();
    let selected = s.catalog.selected.contains(&url);
    let in_library = item.kind == Kind::Track && s.library_url_set().contains(url.as_str());
    let description = if item.detail.is_empty() { item.kind.label().into() } else { format!("{} · {}", item.kind.label(), item.detail) };
    let art = artwork(s, k, &format!("catalog-artwork-{index}"), item.artwork.as_deref(), item_icon(item.kind), FLACORA.artwork.catalog_row, item.kind == Kind::Artist);
    let info = col().gap(FLACORA.spacing.text).grow(1.0).min_w(0.0)
        .child(text(item.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate())
        .child(text(description).font(k.ty.small).color(k.c.text_muted).truncate());
    if item.kind == Kind::Track {
        let quick_url = url.clone();
        let quick_title = item.title.clone();
        let quick = widgets::icon_button(
            k,
            &format!("catalog-quick-{index}"),
            &icons::PLUS,
            "Quick Queue to Downloads",
            move |s: &mut App| s.quick_queue_catalog_track(quick_url.clone(), quick_title.clone()),
        )
        .disabled(s.importing > 0);
        let action = widgets::button(k, &format!("catalog-item-{index}"), if selected { "Selected" } else { "Select" },
            selected.then_some(&icons::CIRCLE_CHECK), if selected { ButtonVariant::Submit } else { ButtonVariant::Outline },
            move |s: &mut App| toggle_track(s, &url));
        let mut row_el = row().w_full().gap(FLACORA.spacing.row).center_y().px(FLACORA.spacing.small).py(FLACORA.spacing.small)
            .child(art)
            .child(info);
        if in_library {
            row_el = row_el.child(
                widgets::status_badge(k, &icons::CIRCLE_CHECK, "In Library", Some(k.c.info))
                    .key(format!("catalog-in-library-{index}")),
            );
        }
        row_el.child(quick).child(action)
    } else {
        let cue = row().gap(FLACORA.spacing.small).center_y().px(FLACORA.spacing.label).a11y_hidden()
            .child(text("Open").font(k.ty.small_medium).color(k.c.text_muted))
            .child(icon(&icons::CHEVRON_RIGHT, FLACORA.icons.inline).color(k.c.icon_muted));
        row()
            .key(format!("catalog-item-{index}"))
            .role("button")
            .label(format!("Open {}", item.title))
            .focusable(true)
            .focus_ring(super::theme::FOCUS_RING_WIDTH, super::theme::FOCUS_RING_OFFSET, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .radius(k.radius.sm)
            .hover(k.c.hover)
            .w_full().gap(FLACORA.spacing.row).center_y().px(FLACORA.spacing.label).py(FLACORA.spacing.label)
            .on_press(move |s: &mut App| s.open_catalog(url.clone(), true))
            .child(art)
            .child(info)
            .child(cue)
    }
}

fn artwork(s: &App, k: &Kit, key: &str, url: Option<&str>, placeholder: &'static Icon, size: f32, round: bool) -> El {
    let radius = if round { size / 2.0 } else { k.radius.md };
    let frame = el().size(size, size).radius(radius).fill(k.c.layer_02).a11y_hidden();
    let frame = if let Some(image) = url.and_then(|url| s.catalog.artwork(url)) {
        frame.pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        let placeholder_frame = frame.center().child(icon(placeholder, size * FLACORA.artwork.placeholder_ratio).color(k.c.icon_muted));
        if let Some(url) = url {
            let url = url.to_owned();
            placeholder_frame.key(url.clone()).on_visible(move |s: &mut App, visible, _| {
                if visible {
                    s.request_catalog_artwork(&url);
                }
            })
        } else {
            placeholder_frame
        }
    };
    el().key(key.to_owned()).size(size, size).child(frame)
}
