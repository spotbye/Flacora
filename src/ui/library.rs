//! Library: every finished download, newest first.

use super::theme::FLACORA;
use super::{theme::Kit, widgets::{self, card, cover}};
use crate::app::{App, LibrarySort, LibraryStatusFilter, Page};
use crate::extensions::Kind;
use crate::icons;
use crate::jobs::now;
use crate::library::Entry;
use crate::services;
use rune::{Cursor, El, GrowthSpec, Tx, col, el, icon, row, text};
use crate::astryx::{ButtonVariant, Collapsible, ControlSize, Divider, IconButton, IconButtonVariant, SegmentedControl, Spinner, TextInput};
use std::{borrow::Cow, rc::Rc};

pub fn page(s: &App, k: &'static Kit) -> El {
    let mut actions = Vec::new();
    let missing_count = s.history.iter().filter(|entry| s.entry_is_missing(entry)).count();
    let has_lyrics_count = s.history.iter().filter(|entry| s.entry_has_lyrics(entry)).count();
    if !s.history.is_empty() {
        let mut search = TextInput::new(&s.search)
            .placeholder("Search the Library")
            .leading_icon(&icons::SEARCH)
            .width(Some(FLACORA.library.search_width))
            .on_edit(|s: &mut App| &mut s.search);
        if !s.search.text.is_empty() {
            search = search.trailing(
                widgets::clear_input_button(k, "search", "Clear Search", |s: &mut App| &mut s.search)
                    .key("library-clear-search"),
            );
        }
        actions.push(search.build(k).key("search"));
        actions.push(
            IconButton::new(&icons::FILE_UP)
                .variant(IconButtonVariant::Ghost)
                .control_size(ControlSize::Large)
                .label("Export Library")
                .help("Export Library to CSV or JSON")
                .on_press_tx(App::export_library)
                .build(k)
                .key("library-export"),
        );
        if s.history.len() > 1 || s.library_select_mode {
            let mut select_btn = widgets::icon_button(
                k,
                "library-select-toggle",
                &icons::LIST_CHECKS,
                "Select Tracks",
                App::toggle_library_select_mode,
            )
            .expanded(s.library_select_mode);
            if s.library_select_mode {
                select_btn = select_btn.fill(k.c.layer_04);
            }
            actions.push(select_btn);
        }
        actions.push(widgets::button(k, "clear-history", "Clear", Some(&icons::TRASH_2), ButtonVariant::Danger, |s: &mut App| s.clear_history()));
    }
    let entries = s.filtered_history();
    let count = s.history.len();
    let filtering = !s.search.text.trim().is_empty()
        || s.active_library_service().is_some()
        || s.active_library_status_filter() != LibraryStatusFilter::All;
    let subtitle = if !s.loaded {
        "Loading your download history…".to_string()
    } else if count == 0 {
        "Your finished downloads appear here.".to_string()
    } else if filtering {
        format!("{} of {count} Download{}", entries.len(), if count == 1 { "" } else { "s" })
    } else if count == 1 {
        "1 Download".to_string()
    } else {
        format!("{count} Downloads")
    };
    let head = widgets::page_header(k, "Library", &subtitle, actions);
    let body = if !s.loaded {
        row()
            .key("library-loading")
            .role("status")
            .gap(FLACORA.spacing.inline)
            .center_y()
            .child(Spinner::new().size(FLACORA.spinner.inline).build(k))
            .child(text("Loading Library…").font(k.ty.body).color(k.c.text_muted))
    } else if s.history.is_empty() {
        widgets::empty_state(
            k,
            "library-empty",
            &icons::LIBRARY,
            "No Downloads Yet",
            "Finished downloads are listed here with their artwork, so you can play them or find them again.",
            Some(widgets::button(k, "go-downloads", "Start Downloading", Some(&icons::DOWNLOAD), ButtonVariant::Submit, |s: &mut App| {
                s.page = Page::Downloads
            })),
        )
    } else {
        list(s, k, entries)
    };
    let mut page = widgets::page("library-page").child(head);
    if s.loaded && s.history.len() > 1 {
        page = page.child(toolbar(s, k));
    }
    if s.loaded && !s.history.is_empty() && (missing_count > 0 || has_lyrics_count > 0) {
        page = page.child(status_toolbar(s, k, missing_count, has_lyrics_count));
    }
    if s.loaded && s.library_select_mode && !s.history.is_empty() {
        page = page.child(batch_bar(s, k));
    }
    page.child(body)
}

fn toolbar(s: &App, k: &Kit) -> El {
    let mut services: Vec<String> = Vec::new();
    for entry in &s.history {
        let name = entry.service.trim();
        if !name.is_empty() && !services.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
            services.push(name.to_owned());
        }
    }
    let mut filters = widgets::filter_group("library-filters", "Library Service").grow(1.0).min_w(0.0);
    let active = s.active_library_service();
    if services.len() > 1 || active.is_some() {
        let total = services.len() + 1;
        filters = filters.child(widgets::filter_button(
            k, "library-filter-all".into(), "All".into(), active.is_none(), None,
            |s: &mut App, _| s.library_service = None,
        ).position_in_set(1, total));
        for (idx, name) in services.into_iter().enumerate() {
            let selected = active.is_some_and(|v| v.eq_ignore_ascii_case(&name));
            let key_slug = name.to_lowercase().replace(' ', "-");
            let value = name.clone();
            filters = filters.child(widgets::filter_button(
                k, format!("library-filter-{key_slug}"), name, selected, None,
                move |s: &mut App, _| s.library_service = Some(value.clone()),
            ).position_in_set(idx + 2, total));
        }
    }
    let sort_labels = LibrarySort::ALL.map(LibrarySort::label);
    let current = LibrarySort::ALL.iter().position(|sort| *sort == s.library_sort);
    let sort = widgets::segmented(
        k,
        "library-sort",
        SegmentedControl::new(&sort_labels, current)
            .label("Sort Library")
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| s.library_sort = LibrarySort::ALL[i]),
    );
    row().key("library-toolbar").w_full().gap(FLACORA.spacing.row).center_y().flex_wrap().child(filters).child(sort)
}

fn status_toolbar(s: &App, k: &Kit, missing_count: usize, has_lyrics_count: usize) -> El {
    let active_status = s.active_library_status_filter();
    let mut items = vec![(LibraryStatusFilter::All, "All Files".to_owned(), "all")];
    if has_lyrics_count > 0 {
        items.push((LibraryStatusFilter::HasLyrics, format!("Has Lyrics ({has_lyrics_count})"), "lyrics"));
    }
    if missing_count > 0 {
        items.push((LibraryStatusFilter::Missing, format!("Missing ({missing_count})"), "missing"));
    }
    let total = items.len();
    let mut group = widgets::filter_group("library-status-filters", "Track Status").grow(1.0).min_w(0.0);
    for (idx, (filter, label, slug)) in items.into_iter().enumerate() {
        group = group.child(
            widgets::filter_button(
                k,
                format!("library-status-{slug}"),
                label,
                active_status == filter,
                None,
                move |s: &mut App, _| s.library_status_filter = filter,
            )
            .position_in_set(idx + 1, total),
        );
    }
    let mut bar = row().key("library-status-toolbar").w_full().gap(FLACORA.spacing.row).center_y().flex_wrap().child(group);
    if missing_count > 0 {
        bar = bar.child(widgets::button(
            k,
            "remove-missing",
            &format!("Remove Missing ({missing_count})"),
            Some(&icons::TRASH_2),
            ButtonVariant::Outline,
            App::remove_missing_entries,
        ));
    }
    bar
}

fn batch_bar(s: &App, k: &Kit) -> El {
    let selected_count = s.library_selected.len();
    let has_lookups = s.installed.iter().any(|i| i.manifest.supports_lookup());
    let mut top = row()
        .w_full()
        .gap(FLACORA.spacing.inline)
        .center_y()
        .flex_wrap()
        .child(
            text(format!("{selected_count} Selected"))
                .key("library-selected-count")
                .font(k.ty.small_medium)
                .color(k.c.text_strong)
                .grow(1.0)
                .min_w(80.0),
        )
        .child(widgets::button(
            k,
            "library-select-visible",
            "Select Visible",
            Some(&icons::CIRCLE_CHECK),
            ButtonVariant::Outline,
            App::select_visible_library,
        ));
    if selected_count > 0 {
        top = top.child(widgets::button(
            k,
            "library-clear-selection",
            "Clear",
            Some(&icons::X),
            ButtonVariant::Ghost,
            App::clear_library_selection,
        ));
        if has_lookups {
            top = top.child(
                widgets::button(
                    k,
                    "library-batch-lookup",
                    "Lyrics & Metadata",
                    Some(&icons::TAG),
                    if s.library_batch_lookup_open { ButtonVariant::Submit } else { ButtonVariant::Outline },
                    |s: &mut App| s.library_batch_lookup_open = !s.library_batch_lookup_open,
                )
                .expanded(s.library_batch_lookup_open),
            );
        }
        top = top.child(widgets::button(
            k,
            "library-batch-remove",
            "Remove Selected",
            Some(&icons::TRASH_2),
            ButtonVariant::Danger,
            App::remove_selected_entries,
        ));
    }
    top = top.child(widgets::button(
        k,
        "library-select-done",
        "Done",
        None,
        ButtonVariant::Ghost,
        App::toggle_library_select_mode,
    ));

    let mut content = col().w_full().gap(FLACORA.spacing.inline).child(top);
    if s.library_batch_lookup_open && selected_count > 0 {
        let mut buttons = row().w_full().gap(FLACORA.spacing.label).center_y().flex_wrap();
        for item in s.installed.iter().filter(|i| i.manifest.supports_lookup() && i.manifest.tag_writer_update().is_none()) {
            let id = item.manifest.id.clone();
            let label = item.manifest.name.strip_suffix(" Web").unwrap_or(&item.manifest.name);
            let disabled = s.importing > 0 || s.removing.contains(&id);
            buttons = buttons.child(
                widgets::button(
                    k,
                    &format!("batch-lookup-{id}"),
                    label,
                    None,
                    ButtonVariant::Outline,
                    move |s: &mut App| s.enqueue_batch_lookup(&id),
                )
                .disabled(disabled),
            );
        }
        content = content
            .child(Divider::horizontal().build(k))
            .child(
                col()
                    .key("library-batch-lookup-providers")
                    .w_full()
                    .gap(FLACORA.spacing.small)
                    .child(text("Run lookup on selected FLACs:").font(k.ty.small_medium).color(k.c.text_muted))
                    .child(buttons),
            );
    }
    card(k, "library-batch-bar", content)
}

fn list(_s: &App, k: &'static Kit, entries: Vec<(usize, &Entry)>) -> El {
    if entries.is_empty() {
        let clear = widgets::button(k, "library-clear-filters", "Clear Filters", Some(&icons::X), ButtonVariant::Outline, |s: &mut App| {
            s.search = rune::TextEdit::default();
            s.library_service = None;
            s.library_status_filter = LibraryStatusFilter::All;
        });
        return widgets::empty_state(k, "library-no-match", &icons::SEARCH, "No Matches", "Nothing in the library matches that search.", Some(clear));
    }
    let now = now();
    let indices: Rc<Vec<usize>> = Rc::new(entries.into_iter().map(|(index, _)| index).collect());
    let key_indices = indices.clone();
    let build_indices = indices.clone();
    let rows = col().key("library-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        indices.len(),
        FLACORA.library.estimated_row_height,
        move |s: &App, position| {
            let index = key_indices[position];
            let date = s.history.get(index).map_or(0, |entry| entry.date);
            Cow::Owned(format!("library-row-{date}-{index}"))
        },
        move |s: &App, position| {
            let index = build_indices[position];
            let Some(entry) = s.history.get(index) else {
                return el();
            };
            let mut wrap = col().w_full();
            if position > 0 {
                wrap = wrap.child(Divider::horizontal().build(k));
            }
            wrap.child(entry_row(s, k, index, entry, now))
        },
    );
    // The card keeps its own height; the column around it scrolls.
    widgets::page_scroll(k, col().key("library-scroll").child(card(k, "library-list", rows)))
}

fn entry_row(s: &App, k: &Kit, index: usize, entry: &Entry, now: u64) -> El {
    let mut subtitle = vec![entry.artist.clone()];
    if !entry.album.is_empty() {
        subtitle.push(entry.album.clone());
    }
    subtitle.retain(|t| !t.is_empty());
    let mut info = col().gap(FLACORA.spacing.text).grow(1.0).shrink().min_w(160.0).child(text(entry.title.clone()).font(k.ty.body_medium).color(k.c.text_strong).truncate());
    if !subtitle.is_empty() {
        info = info.child(text(subtitle.join(" · ")).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    let file_info = entry.main_file().and_then(|file| s.library_files.get(file));
    let missing = file_info.is_some_and(|info| !info.exists);
    let mut meta = row().gap(FLACORA.spacing.inline).center_y().shrink().min_w(0.0).justify_end();
    if missing {
        meta = meta.child(
            widgets::status_badge(k, &icons::TRIANGLE_ALERT, "Missing", Some(k.c.warning))
                .key(format!("missing-entry-{index}"))
                .help("Audio file was moved or deleted from disk."),
        );
    }
    if let Some(info) = file_info {
        for (label, sidecar_path) in &info.sidecars {
            let path = sidecar_path.clone();
            let slug = label.to_lowercase();
            let filename = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            meta = meta.child(
                row()
                    .key(format!("sidecar-{index}-{slug}"))
                    .role("button")
                    .label(format!("Open {label}"))
                    .help(format!("Open {filename}"))
                    .focusable(true)
                    .focus_ring(super::theme::FOCUS_RING_WIDTH, super::theme::FOCUS_RING_OFFSET, k.c.border_focus)
                    .cursor(Cursor::Pointer)
                    .h(FLACORA.badge.height)
                    .px(FLACORA.badge.sidecar_padding_x)
                    .center()
                    .radius(FLACORA.badge.radius)
                    .fill(k.c.layer_02)
                    .hover(k.c.layer_04)
                    .on_press(move |s: &mut App| s.open(&path))
                    .child(text(label.clone()).font(k.ty.text_xsmall_medium).color(k.c.text_base).a11y_hidden()),
            );
        }
    }
    if !entry.quality.is_empty() {
        meta = meta.child(text(entry.quality.clone()).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    let meta = meta.child(source_icon(s, k, entry));
    let mut actions = row().gap(FLACORA.spacing.tight).center_y();
    let has_lookups = s.installed.iter().any(|i| i.manifest.supports_lookup());
    let lookup_target = entry.flac_file().filter(|_| has_lookups && !missing).map(|file| (entry.date, file.to_owned()));
    let lookup_open = lookup_target.is_some() && lookup_target == s.lookup_picker;
    let inspect_target = entry.main_file().filter(|_| !missing).map(|file| (entry.date, file.to_owned()));
    let inspect_open = inspect_target.is_some() && inspect_target == s.track_inspector;
    if let Some(target) = lookup_target {
        let mut trigger = widgets::icon_button(k, &format!("lookup-{index}"), &icons::TAG, "Lyrics & Metadata", move |s: &mut App| {
            s.track_inspector = None;
            s.lookup_picker = (s.lookup_picker.as_ref() != Some(&target)).then(|| target.clone());
        })
        .expanded(lookup_open);
        if lookup_open {
            trigger = trigger.fill(k.c.layer_04);
        }
        actions = actions.child(trigger);
    }
    if let Some((date, file)) = &inspect_target {
        let (date, file) = (*date, file.clone());
        let mut trigger = widgets::icon_button(k, &format!("inspect-entry-{index}"), &icons::CIRCLE_INFO, "Track Details", move |s: &mut App| {
            s.toggle_track_inspector(date, file.clone());
        })
        .expanded(inspect_open);
        if inspect_open {
            trigger = trigger.fill(k.c.layer_04);
        }
        actions = actions.child(trigger);
    }
    if let Some(file) = entry.main_file() {
        let (play, reveal) = (file.to_owned(), file.to_owned());
        if missing && !entry.url.trim().is_empty() {
            actions = actions.child(widgets::icon_button(
                k,
                &format!("redownload-entry-{index}"),
                &icons::REFRESH_CW,
                "Re-download",
                move |s: &mut App| s.redownload_entry(index),
            ));
        } else {
            actions = actions.child(
                widgets::icon_button(k, &format!("play-entry-{index}"), &icons::PLAY, "Play", move |s: &mut App| s.open(&play))
                    .disabled(missing),
            );
        }
        actions = actions.child(
            widgets::icon_button(k, &format!("reveal-entry-{index}"), &icons::FOLDER, "Show in Folder", move |s: &mut App| {
                s.reveal(&reveal)
            })
            .disabled(missing),
        );
    }
    actions = actions.child(widgets::danger_icon_button(k, &format!("remove-entry-{index}"), &icons::X, "Remove from Library", move |s: &mut App| {
        s.remove_entry(index)
    }));
    let mut artwork = cover(k, s.image(entry.cover.as_deref()), FLACORA.artwork.library_cover);
    if let Some(path) = entry.cover.as_deref()
        && s.image(Some(path)).is_none()
    {
        let path = path.to_owned();
        artwork = artwork.on_visible(move |s: &mut App, visible, _| {
            if visible {
                s.request_history_cover(&path);
            }
        });
    }
    let mut head = row()
        .key(format!("entry-{index}"))
        .w_full()
        .px(FLACORA.spacing.small)
        .py(FLACORA.spacing.row)
        .gap(FLACORA.spacing.media)
        .center_y();
    if s.library_select_mode {
        let selected = s.library_selected.contains(&App::entry_selection_key(entry));
        let mut checkbox = el()
            .key(format!("select-entry-{index}"))
            .role("checkbox")
            .label(format!("Select {}", entry.title))
            .checked(selected)
            .focusable(true)
            .focus_ring(super::theme::FOCUS_RING_WIDTH, super::theme::FOCUS_RING_OFFSET, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .size(20.0, 20.0)
            .radius(k.radius.xs)
            .center()
            .fill(if selected { k.c.bg_accent } else { k.c.bg_base })
            .border(
                FLACORA.border.normal,
                if selected { k.c.bg_accent } else { k.c.border_strong },
            )
            .hover(if selected { k.c.bg_accent } else { k.c.hover })
            .on_press(move |s: &mut App| s.toggle_library_entry_selected(index));
        if selected {
            checkbox = checkbox.child(icon(&icons::CHECK, FLACORA.icons.small).color(k.c.submit_fg).a11y_hidden());
        }
        head = head.child(checkbox);
    }
    let head = head
        .child(artwork)
        .child(info)
        .child(meta)
        .child(text(widgets::ago(entry.date, now)).font(k.ty.text_xsmall).color(k.c.text_faint).w(FLACORA.library.timestamp_width).truncate())
        .child(actions);
    if lookup_open {
        col().w_full().child(head).child(lookup_providers(s, k, index))
    } else if inspect_open && let Some((_, file)) = &inspect_target {
        col().w_full().child(head).child(track_inspector_panel(s, k, index, entry, file))
    } else {
        head
    }
}

fn track_inspector_panel(s: &App, k: &Kit, index: usize, entry: &Entry, file: &str) -> El {
    let mut details = col().w_full().gap(FLACORA.spacing.small).pt(FLACORA.spacing.label);
    let spec_row = |label: &str, value: String| {
        row()
            .w_full()
            .gap(FLACORA.spacing.inline)
            .items_start()
            .child(text(label.to_owned()).font(k.ty.small_medium).color(k.c.text_muted).w(FLACORA.library.lookup_label_width))
            .child(text(value).font(k.ty.small).color(k.c.text_base).wrap().grow(1.0).min_w(0.0))
    };
    details = details.child(spec_row("File", file.to_owned()));
    match s.track_specs.get(file) {
        None => {
            details = details.child(
                row()
                    .role("status")
                    .gap(FLACORA.spacing.inline)
                    .center_y()
                    .child(Spinner::new().size(FLACORA.spinner.inline).build(k))
                    .child(text("Reading FLAC tags…").font(k.ty.small).color(k.c.text_muted)),
            );
        }
        Some(None) => {
            if !entry.quality.is_empty() {
                details = details.child(spec_row("Quality", entry.quality.clone()));
            }
            if !entry.extension.is_empty() {
                details = details.child(spec_row("Downloaded With", entry.extension.clone()));
            }
        }
        Some(Some(specs)) => {
            let mut format_parts = Vec::new();
            if let (Some(bits), Some(rate)) = (specs.bits_per_sample, specs.sample_rate) {
                let khz = rate as f64 / 1000.0;
                let khz_str = if (khz.fract()).abs() < 1e-6 { format!("{khz:.0}") } else { format!("{khz:.1}") };
                format_parts.push(format!("{bits}-bit / {khz_str} kHz"));
            }
            if let Some(ch) = specs.channels {
                format_parts.push(match ch {
                    1 => "Mono".into(),
                    2 => "Stereo".into(),
                    n => format!("{n} ch"),
                });
            }
            if let Some(secs) = specs.duration_secs {
                format_parts.push(format!("{}:{:02}", secs / 60, secs % 60));
            }
            let mb = specs.size_bytes as f64 / (1024.0 * 1024.0);
            format_parts.push(format!("{mb:.2} MB"));
            details = details.child(spec_row("Audio", format_parts.join(" · ")));
            if let Some(cover) = &specs.cover_summary {
                details = details.child(spec_row("Artwork", cover.clone()));
            }
            if !entry.extension.is_empty() {
                details = details.child(spec_row("Downloaded With", entry.extension.clone()));
            }
            if !specs.tags.is_empty() {
                details = details.child(Divider::horizontal().build(k));
                for (tag_label, tag_value) in &specs.tags {
                    details = details.child(spec_row(tag_label, tag_value.clone()));
                }
            }
        }
    }
    let disclosure = Collapsible::new("Track Details", true)
        .on_toggle_tx(move |s: &mut App, tx: &mut Tx| {
            s.track_inspector = None;
            tx.focus_when_shown(format!("inspect-entry-{index}"), false);
        })
        .child(details)
        .build(k)
        .key(format!("inspect-disclosure-{index}"));
    let panel = col()
        .key(format!("track-inspector-{index}"))
        .w_full()
        .padding(FLACORA.spacing.row)
        .radius(k.radius.md)
        .fill(k.c.bg_deep)
        .border(FLACORA.border.normal, k.c.border_base)
        .child(disclosure);
    col().w_full().px(FLACORA.spacing.small).pb(FLACORA.spacing.row).child(panel)
}

fn source_icon(s: &App, k: &Kit, entry: &Entry) -> El {
    let name = entry.service.trim();
    let service = services::by_name(name);
    let label = service.map_or(if name.is_empty() { "Audio Source" } else { name }, |service| service.name);
    service.and_then(|service| widgets::provider_logo(s, service, FLACORA.artwork.provider_source))
        .unwrap_or_else(|| icon(&icons::LINK, FLACORA.icons.standard).key("source-link").color(k.c.icon_muted))
        .role("img")
        .label(label.to_owned())
        .help(label.to_owned())
}

fn lookup_providers(s: &App, k: &Kit, index: usize) -> El {
    let mut lyrics = Vec::new();
    let mut tags = Vec::new();
    let mut json = Vec::new();
    let mut metadata = Vec::new();
    let mut updates = Vec::new();
    for item in s.installed.iter().filter(|item| item.manifest.supports_lookup()) {
        let id = item.manifest.id.clone();
        let key = format!("lookup-{index}-{id}");
        let label = item.manifest.name.strip_suffix(" Web").unwrap_or(&item.manifest.name);
        let (group, help) = if item.manifest.kind == Kind::Lyrics {
            (&mut lyrics, format!("Get lyrics from {label}"))
        } else if let Some(message) = item.manifest.tag_writer_update() {
            (&mut updates, message)
        } else if item.manifest.can_write_tags() {
            (&mut tags, format!("Write {label} metadata to this FLAC"))
        } else if id == "musicbrainz" {
            (&mut json, "Export MusicBrainz metadata as a JSON file".into())
        } else {
            (&mut metadata, format!("Export {label} metadata"))
        };
        let disabled = s.importing > 0 || s.removing.contains(&id);
        group.push(widgets::button(k, &key, label, None, ButtonVariant::Outline,
            move |s: &mut App| s.enqueue_lookup(index, &id),
        ).disabled(disabled).help(help));
    }

    let mut providers = col().w_full().gap(FLACORA.spacing.inline);
    let mut first = true;
    for (title, glyph, buttons) in [
        ("Get Lyrics", &icons::MIC_VOCAL, lyrics),
        ("Tag FLAC", &icons::TAG, tags),
        ("Export JSON", &icons::FILE_UP, json),
        ("Export Metadata", &icons::FILE_UP, metadata),
        ("Update Required", &icons::TRIANGLE_ALERT, updates),
    ] {
        if buttons.is_empty() {
            continue;
        }
        if !first {
            providers = providers.child(Divider::horizontal().build(k));
        }
        first = false;
        let label = row().w(FLACORA.library.lookup_label_width).h(FLACORA.library.lookup_label_height).gap(FLACORA.spacing.label).center_y()
            .child(icon(glyph, FLACORA.icons.small).color(k.c.icon_muted))
            .child(text(title).font(k.ty.small_medium).color(k.c.text_muted));
        providers = providers.child(row().role("group").label(title).w_full().gap(FLACORA.spacing.inline).items_start()
            .child(label).child(row().gap(FLACORA.spacing.label).center_y().flex_wrap().grow(1.0).min_w(0.0).children(buttons)));
    }
    let disclosure = Collapsible::new("Lyrics & Metadata", true)
        .on_toggle_tx(move |s: &mut App, tx: &mut Tx| {
            s.lookup_picker = None;
            tx.focus_when_shown(format!("lookup-{index}"), false);
        })
        .child(providers.pt(FLACORA.spacing.label))
        .build(k).key(format!("lookup-disclosure-{index}"));
    let panel = col()
        .key(format!("lookup-providers-{index}"))
        .w_full()
        .padding(FLACORA.spacing.row)
        .radius(k.radius.md)
        .fill(k.c.bg_deep)
        .border(FLACORA.border.normal, k.c.border_base)
        .child(disclosure);
    col().w_full().px(FLACORA.spacing.small).pb(FLACORA.spacing.row).child(panel)
}
