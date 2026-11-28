//! Downloads: the link box with its route, then the queue.

use super::theme::FLACORA;
use super::theme::Kit;
use super::widgets::{self, badge, card, cover, section, service_chip, status_badge, title_case_label};
use crate::app::{App, ConfirmAction, Page, QueueFilter, RESOLVER_KEY};
use crate::{icons, store};
use crate::jobs::{Job, Status};
use rune::{El, GrowthSpec, ScrollSpec, Tx, col, el, icon, row, text};
use crate::astryx::{Button, ButtonVariant, ControlSize, Menu, MenuItem, Placement, ScrollBar, SegmentedControl, Spinner, TextInput};
use std::{borrow::Cow, rc::Rc};

pub fn page(s: &App, k: &'static Kit, overlays: &mut Vec<El>) -> El {
    let active = s.active_jobs();
    let failed_count = s.jobs.iter().filter(|j| matches!(j.status, Status::Failed(_))).count();
    let cancelled_count = s.jobs.iter().filter(|j| matches!(j.status, Status::Cancelled)).count();
    let mut actions = Vec::new();
    if active > 1 {
        actions.push(widgets::button(k, "cancel-all", "Cancel All", Some(&icons::SQUARE), ButtonVariant::Outline, |s: &mut App| {
            s.confirm_action = Some(ConfirmAction::CancelAll);
        }));
    }
    if failed_count > 0 || cancelled_count > 0 {
        let label = match (failed_count > 0, cancelled_count > 0) {
            (true, true) => "Retry Failed & Cancelled",
            (true, false) => "Retry Failed",
            (false, true) => "Retry Cancelled",
            (false, false) => unreachable!(),
        };
        actions.push(widgets::button(k, "retry-failed", label, Some(&icons::REFRESH_CW), ButtonVariant::Outline, |s: &mut App| {
            s.retry_failed_jobs()
        }));
    }
    let head = widgets::page_header(k, "Downloads", "Paste links from your music services. They download one after another.", actions);
    let body = if s.jobs.is_empty() { empty(s, k) } else { queue(s, k, failed_count > 0, overlays) };
    if s.confirm_action == Some(ConfirmAction::CancelAll) && active > 0 {
        overlays.push(cancel_all_dialog(s, k));
    }
    let mut page = widgets::page("downloads-page").child(head);
    if !s.loaded || !s.installed.is_empty() || !s.jobs.is_empty() {
        page = page.child(composer(s, k));
    }
    page.child(body)
}

fn cancel_all_dialog(s: &App, k: &Kit) -> El {
    let running = s.jobs.iter().filter(|job| job.status == Status::Running).count();
    let queued = s.jobs.iter().filter(|job| job.status == Status::Queued).count();
    let choosing = s.jobs.iter().filter(|job| job.status == Status::Choosing).count();
    let total = running + queued + choosing;
    let mut counts = format!("{running} active, {queued} queued");
    if choosing > 0 {
        counts.push_str(&format!(", {choosing} awaiting a service choice"));
    }
    let detail = if total > 0 {
        format!("{total} job{} will be canceled ({counts}).", if total == 1 { "" } else { "s" })
    } else {
        "There are no jobs to cancel.".into()
    };
    let keep = widgets::button(k, "keep-downloading", "Keep Downloading", None, ButtonVariant::Neutral,
        |s: &mut App| s.confirm_action = None);
    let cancel = widgets::button(k, "confirm-cancel-all", "Cancel All", Some(&icons::SQUARE), ButtonVariant::Danger, |s: &mut App| {
        s.confirm_action = None;
        s.cancel_all_jobs();
    }).disabled(total == 0);
    widgets::confirmation_dialog(k, "cancel-all", "Cancel all jobs?", detail, "cancel-all", keep, cancel)
}

/// Valid URLs currently in the link box.
fn pasted_links(s: &App) -> Vec<&str> {
    s.url.text.split_whitespace().filter(|w| crate::extensions::url_host(w).is_some()).collect()
}

fn duplicate_count(s: &App, links: &[&str]) -> usize {
    if links.is_empty() || s.history.is_empty() {
        return 0;
    }
    let existing = s.library_url_set();
    links.iter().filter(|url| existing.contains(**url)).count()
}

/// The link box, its route and the destination.
fn composer(s: &App, k: &Kit) -> El {
    let links = pasted_links(s);
    let service = s.service();
    let collection = links.len() == 1 && crate::catalog::is_catalog_url(links[0]);
    let mut field = TextInput::new(&s.url)
        .placeholder("Paste Track or Collection Links…")
        .leading_icon(&icons::LINK)
        .large()
        .on_edit(|s: &mut App| &mut s.url)
        .on_submit(|s: &mut App, _: &mut Tx| s.enqueue());
    if !s.url.text.is_empty() {
        let mut accessories = row().gap(FLACORA.spacing.inline).center_y();
        if links.len() > 1 {
            accessories = accessories.child(badge(k, format!("{} Links", links.len())).key("batch-summary"));
        } else if let Some(service) = service {
            accessories = accessories.child(service_chip(k, Some(service), service.name));
        }
        let clear = widgets::clear_input_button(k, "url", "Clear Links", |s: &mut App| {
            s.set_batch_service(None);
            &mut s.url
        }).key("downloads-clear-links");
        field = field.trailing(accessories.child(clear));
    }
    let can_add = s.importing == 0 && (collection || s.selected().is_some() || s.resolver().is_some() || links.len() > 1);
    let (label, glyph) = if collection {
        ("Browse".to_owned(), &icons::SEARCH)
    } else if links.len() > 1 {
        (format!("Download ({})", links.len()), &icons::DOWNLOAD)
    } else {
        ("Download".to_owned(), &icons::DOWNLOAD)
    };
    let add = Button::new(label)
        .icon(glyph)
        .variant(ButtonVariant::Submit)
        .control_size(ControlSize::ExtraLarge)
        .disabled(!can_add)
        .on_press(|s: &mut App| s.enqueue())
        .build(k)
        .key("download");
    let input = row().key("link").w_full().gap(FLACORA.spacing.row).center_y().child(field.build(k).key("url").grow(1.0)).child(add);

    let path = s.settings.output.to_string_lossy().into_owned();
    let destination = Button::new(path.clone())
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Small)
        .icon(&icons::FOLDER)
        .label_with(move |font, ink| text(path).font(font).color(ink).truncate().shrink())
        .help("Change the Download Folder in Settings")
        .on_press(|s: &mut App| s.page = Page::Settings)
        .build(k)
        .key("destination")
        .max_w(FLACORA.queue.destination_max_width)
        .shrink()
        .min_w(0.0);
    let body = col()
        .gap(FLACORA.spacing.group)
        .child(input)
        .child(row().w_full().gap(FLACORA.spacing.section).center_y().flex_wrap().child(route(s, k, &links, collection)).child(destination))
        .children(batch_service_picker(s, k, &links));
    card(k, "composer", body)
}

fn batch_service_picker(s: &App, k: &Kit, links: &[&str]) -> Option<El> {
    if !s.loaded || links.len() < 2 || !links.iter().any(|url| {
        !crate::catalog::is_catalog_url(url) && crate::extensions::url_host(url)
            .is_some_and(|host| s.resolver_for(&host).is_some())
    }) {
        return None;
    }
    let destinations: Vec<_> = crate::services::SERVICES.iter()
        .filter(|service| s.batch_has_downloader(service)).collect();
    if destinations.is_empty() {
        return None;
    }
    let selected = s.batch_service();
    let total = destinations.len() + 1;
    let mut picker = widgets::filter_group("batch-services", "Service for Resolved Tracks")
        .child(widgets::filter_button(k, "batch-service-manual".into(), "Choose per Track".into(),
            selected.is_none(), None, |s: &mut App, _| s.set_batch_service(None)).position_in_set(1, total));
    for (idx, service) in destinations.into_iter().enumerate() {
        picker = picker.child(widgets::filter_button(
            k, format!("batch-service-{}", service.key), service.name.into(), selected == Some(service),
            widgets::provider_logo(s, service, FLACORA.artwork.provider_inline), move |s: &mut App, _| s.set_batch_service(Some(service)),
        ).position_in_set(idx + 2, total));
    }
    Some(row().w_full().flex_wrap().center_y().gap(FLACORA.spacing.inline)
        .child(text("Service for Resolved Tracks").font(k.ty.small_medium).color(k.c.text_muted))
        .child(picker))
}

fn duplicate_hint_badge(s: &App, k: &Kit, links: &[&str]) -> Option<El> {
    let dups = duplicate_count(s, links);
    if dups == 0 {
        return None;
    }
    let label = if links.len() == 1 { "In Library".to_owned() } else { format!("{dups} in Library") };
    Some(
        status_badge(k, &icons::CIRCLE_CHECK, &label, Some(k.c.info))
            .key("duplicate-hint")
            .help("Already in Library. Click Download to download again."),
    )
}

/// Which extension takes the link, and its options.
fn route(s: &App, k: &Kit, links: &[&str], collection: bool) -> El {
    let muted = |t: &str| text(t.to_owned()).font(k.ty.small).color(k.c.text_muted).truncate().shrink();
    let line = row().key("route").min_h(FLACORA.queue.route_min_height).gap(FLACORA.spacing.row).center_y().flex_wrap().grow(1.0).shrink().min_w(0.0);
    if !s.loaded {
        return line.role("status").child(Spinner::new().size(FLACORA.spinner.inline).build(k)).child(muted("Loading Extensions…"));
    }
    if collection {
        return line.child(muted("Open this music page and select individual tracks in Catalog."));
    }
    let Some(host) = s.host() else {
        return line.child(
            text(direct_services_hint(s))
                .font(k.ty.small)
                .color(k.c.text_muted)
                .wrap()
                .shrink(),
        );
    };
    if s.selected().is_none()
        && let Some(resolver) = s.resolver()
    {
        let resolvers = s.resolvers();
        let line = line.child(muted("Using"));
        let line = if resolvers.len() > 1 {
            let names: Vec<String> = resolvers.iter().map(|i| i.manifest.name.clone()).collect();
            let ids: Vec<String> = resolvers.iter().map(|i| i.manifest.id.clone()).collect();
            let current = ids.iter().position(|id| *id == resolver.manifest.id);
            line.child(
                widgets::segmented(k, "resolver-pick", SegmentedControl::new(&names, current)
                    .label("Link Resolver")
                    .on_pick(move |s: &mut App, i: usize, _: &mut Tx| s.set_preferred(RESOLVER_KEY, ids.get(i).cloned()))),
            )
        } else {
            line.child(badge(k, &resolver.manifest.name).key("resolver"))
        };
        return line
            .child(icon(&icons::ARROW_RIGHT, FLACORA.icons.small).color(k.c.icon_faint))
            .child(text("finds it on a service you can download from").font(k.ty.small).color(k.c.text_muted).wrap().shrink())
            .children(duplicate_hint_badge(s, k, links));
    }
    let Some(selected) = s.selected() else {
        return line
            .child(icon(&icons::TRIANGLE_ALERT, FLACORA.icons.warning).color(k.c.warning.fg))
            .child(text(format!("No installed extension handles {host}")).font(k.ty.small_medium).color(k.c.warning.fg).wrap().shrink())
            .child(widgets::button(k, "open-extensions", "Go to Extensions", None, ButtonVariant::Ghost, |s: &mut App| {
                s.page = Page::Extensions
            }));
    };

    let mut line = line.child(muted("Using"));
    let candidates = s.candidates();
    if candidates.len() > 1 {
        let names: Vec<String> = candidates.iter().map(|i| i.manifest.name.clone()).collect();
        let ids: Vec<String> = candidates.iter().map(|i| i.manifest.id.clone()).collect();
        let current = ids.iter().position(|id| *id == selected.manifest.id);
        let key = App::service_key(&host);
        line = line.child(
            widgets::segmented(k, "extension-pick", SegmentedControl::new(&names, current)
                .label("Downloader")
                .on_pick(move |s: &mut App, i: usize, _: &mut Tx| s.set_preferred(&key, ids.get(i).cloned()))),
        );
    } else {
        line = line.child(badge(k, &selected.manifest.name).key("extension"));
    }
    for option in &selected.manifest.options {
        line = line.child(muted(&title_case_label(&option.label))).child(
            widgets::extension_option(k, s, &selected.manifest.id, option, &format!("option-{}", option.key)),
        );
    }
    if !store::CAN_DOWNLOAD_FFMPEG && selected.manifest.requires.iter().any(|r| r == "ffmpeg") && s.tools.as_ref().is_some_and(|t| t.ffmpeg.is_none()) {
        line = line
            .child(icon(&icons::TRIANGLE_ALERT, FLACORA.icons.inline).color(k.c.warning.fg))
            .child(text("Needs FFmpeg").font(k.ty.small).color(k.c.warning.fg));
    }
    line.children(duplicate_hint_badge(s, k, links))
}

fn direct_services_hint(s: &App) -> String {
    let direct: Vec<&str> = crate::services::SERVICES[..crate::services::PRIMARY_DOWNLOAD_COUNT]
        .iter()
        .filter(|service| !s.installed_downloaders_for(service.primary_host()).is_empty())
        .map(|service| service.name)
        .collect();
    let has_resolver = s.installed.iter().any(|i| i.manifest.kind == crate::extensions::Kind::Resolver);
    let direct_part = match direct.as_slice() {
        [] => "Install a downloader extension for direct lossless downloads.".to_owned(),
        [only] => format!("{only} links download directly."),
        [head @ .., last] => format!("{} and {last} links download directly.", head.join(", ")),
    };
    if has_resolver {
        format!("{direct_part} Spotify, Apple Music and YouTube use a link resolver.")
    } else {
        direct_part
    }
}

fn empty(s: &App, k: &Kit) -> El {
    let (title, body) = if s.installed.is_empty() && s.loaded {
        (
            "Install an Extension to Start",
            format!("Drop a .{} package into the window. Installed extensions appear on the Extensions page.", store::PACKAGE),
        )
    } else {
        ("Nothing in the Queue", "Paste a link above and press Enter. Several links at once are queued in order.".to_owned())
    };
    let action = (s.installed.is_empty() && s.loaded).then(|| {
        widgets::button(k, "go-extensions", "Go to Extensions", Some(&icons::PUZZLE), ButtonVariant::Submit, |s: &mut App| {
            s.page = Page::Extensions
        })
    });
    widgets::empty_state(k, "queue-empty", &icons::LIST_MUSIC, title, &body, action)
}

fn matches_queue_filter(job: &Job, filter: QueueFilter) -> bool {
    match filter {
        QueueFilter::All => true,
        QueueFilter::Active => matches!(job.status, Status::Running | Status::Queued),
        QueueFilter::Choosing => job.status == Status::Choosing,
        QueueFilter::Failed => matches!(job.status, Status::Failed(_) | Status::Cancelled),
        QueueFilter::Done => job.status == Status::Done,
    }
}

fn queue(s: &App, k: &'static Kit, has_failures: bool, overlays: &mut Vec<El>) -> El {
    let active = s.active_jobs();
    let queued_count = s.jobs.iter().filter(|j| j.status == Status::Queued).count();
    let choosing_count = s.jobs.iter().filter(|j| j.status == Status::Choosing).count();
    let done_count = s.jobs.iter().filter(|j| j.status == Status::Done).count();
    let failed_or_cancelled = s.jobs.iter().filter(|j| matches!(j.status, Status::Failed(_) | Status::Cancelled)).count();
    let active_work = s.jobs.iter().filter(|j| matches!(j.status, Status::Running | Status::Queued)).count();
    let finished = s.jobs.len() - active;
    let mut summary = Vec::new();
    if done_count > 0 && s.jobs.len() > 1 {
        summary.push(format!("{done_count} / {} Done", s.jobs.len()));
    }
    if active > 0 {
        summary.push(format!("{active} Active"));
    }
    if finished > 0 && (done_count == 0 || s.jobs.len() == 1) {
        summary.push(format!("{finished} Finished"));
    }
    let mut queue_left = rune::row()
        .gap(FLACORA.spacing.inline)
        .center_y()
        .grow(1.0)
        .child(section(k, "Queue", None))
        .child(text(summary.join(" · ")).font(k.ty.text_xsmall).color(k.c.text_faint));
    if s.queue_paused {
        queue_left = queue_left.child(
            status_badge(k, &icons::PAUSE, "Paused", Some(k.c.warning)).key("queue-paused-badge"),
        );
    }
    let mut queue_head = rune::row().w_full().gap(FLACORA.spacing.inline).center_y().child(queue_left);
    if queued_count > 0 || (s.queue_paused && active > 0) {
        let (label, glyph, variant) = if s.queue_paused {
            ("Resume Queue", &icons::PLAY, ButtonVariant::Submit)
        } else {
            ("Pause Queue", &icons::PAUSE, ButtonVariant::Outline)
        };
        queue_head = queue_head.child(
            Button::new(label)
                .icon(glyph)
                .variant(variant)
                .control_size(ControlSize::Small)
                .help(if s.queue_paused { "Resume starting queued downloads" } else { "Pause starting the next queued download" })
                .on_press(|s: &mut App| s.toggle_queue_pause())
                .compact().build(k)
                .key("toggle-pause"),
        );
    }
    if finished > 0 {
        queue_head = queue_head.child(queue_menu(s, k, has_failures, overlays));
    }

    let buckets = [
        (QueueFilter::Active, "Active", "active", active_work),
        (QueueFilter::Choosing, "Choosing", "choosing", choosing_count),
        (QueueFilter::Failed, "Failed", "failed", failed_or_cancelled),
        (QueueFilter::Done, "Done", "done", done_count),
    ];
    let present: Vec<_> = buckets.into_iter().filter(|(_, _, _, n)| *n > 0).collect();
    let effective_filter = if present.iter().any(|(f, _, _, _)| *f == s.queue_filter) {
        s.queue_filter
    } else {
        QueueFilter::All
    };
    let filter_bar = (present.len() > 1).then(|| {
        let total = present.len() + 1;
        let mut group = widgets::filter_group("queue-filters", "Queue Status").child(
            widgets::filter_button(
                k,
                "queue-filter-all".into(),
                format!("All ({})", s.jobs.len()),
                effective_filter == QueueFilter::All,
                None,
                |s: &mut App, _| s.queue_filter = QueueFilter::All,
            )
            .position_in_set(1, total),
        );
        for (idx, (filter, label, slug, count)) in present.into_iter().enumerate() {
            group = group.child(
                widgets::filter_button(
                    k,
                    format!("queue-filter-{slug}"),
                    format!("{label} ({count})"),
                    effective_filter == filter,
                    None,
                    move |s: &mut App, _| s.queue_filter = filter,
                )
                .position_in_set(idx + 2, total),
            );
        }
        group
    });

    let bulk_choose = (choosing_count > 1).then(|| bulk_choose_bar(s, k, choosing_count)).flatten();
    let first_queued_id = s.jobs.iter().find(|j| j.status == Status::Queued).map(|j| j.id);
    let indices: Rc<Vec<usize>> = Rc::new(
        s.jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| matches_queue_filter(job, effective_filter))
            .map(|(i, _)| i)
            .collect(),
    );
    let key_indices = indices.clone();
    let build_indices = indices.clone();
    let rows = col().key("queue-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        indices.len(),
        FLACORA.queue.estimated_row_height,
        move |s: &App, position| {
            let index = key_indices[position];
            s.jobs
                .get(index)
                .map(|job| Cow::Owned(format!("job-row-{}", job.id)))
                .unwrap_or_else(|| Cow::Owned(format!("job-row-missing-{position}")))
        },
        move |s: &App, position| {
            let index = build_indices[position];
            let Some(job) = s.jobs.get(index) else {
                return el();
            };
            let card = job_card(s, k, job, first_queued_id);
            col().w_full().pt(if position > 0 { FLACORA.spacing.row } else { 0.0 }).child(card)
        },
    );
    let mut list = col().key("queue").gap(FLACORA.spacing.row).child(queue_head);
    if let Some(bar) = filter_bar {
        list = list.child(bar);
    }
    if let Some(bulk) = bulk_choose {
        list = list.child(bulk);
    }
    list = list.child(rows);
    widgets::page_scroll(k, list)
}

fn bulk_choose_bar(s: &App, k: &Kit, choosing_count: usize) -> Option<El> {
    let mut services: Vec<&'static crate::services::Service> = Vec::new();
    for job in s.jobs.iter().filter(|j| j.status == Status::Choosing) {
        for choice in &job.choices {
            if !services.contains(&choice.service) {
                services.push(choice.service);
            }
        }
    }
    if services.is_empty() {
        return None;
    }
    let mut buttons = row().gap(FLACORA.spacing.small).center_y().flex_wrap();
    for service in services {
        let logo = widgets::provider_logo(s, service, FLACORA.artwork.provider_inline);
        let label = service.name.to_owned();
        let mut button = Button::new(label.clone())
            .variant(ButtonVariant::Neutral)
            .control_size(ControlSize::Small)
            .help(format!("Download all matching tracks from {}", service.name))
            .on_press(move |s: &mut App| s.choose_all_with_service(service));
        if let Some(logo) = logo {
            button = button.label_with(move |font, ink| {
                row().gap(FLACORA.spacing.label).center_y().child(logo.a11y_hidden()).child(text(label).font(font).color(ink))
            });
        }
        buttons = buttons.child(button.compact().build(k).key(format!("choose-all-{}", service.key)));
    }
    Some(card(
        k,
        "choosing-bulk-bar",
        row()
            .w_full()
            .gap(FLACORA.spacing.inline)
            .center_y()
            .flex_wrap()
            .child(
                text(format!("Apply service to {choosing_count} waiting tracks:"))
                    .font(k.ty.small_medium)
                    .color(k.c.text_strong),
            )
            .child(buttons),
    ))
}

fn queue_menu(s: &App, k: &Kit, has_failures: bool, overlays: &mut Vec<El>) -> El {
    let trigger = widgets::icon_button(k, "queue-more", &icons::ELLIPSIS, "More Queue Actions", |s: &mut App| {
        s.queue_menu_open = !s.queue_menu_open;
    });
    let mut menu = Menu::new("queue-actions").trigger(trigger).open(s.queue_menu_open)
        .placement(Placement::BottomEnd).min_width(FLACORA.queue.menu_min_width).on_close(|s: &mut App| s.queue_menu_open = false);
    if has_failures {
        menu = menu.item(MenuItem::new("Copy Failed Links").icon(&icons::COPY).on_select_tx(|s: &mut App, tx| {
            s.queue_menu_open = false;
            let urls = s.failed_urls();
            if !urls.is_empty() {
                let count = urls.len();
                tx.copy(urls.join("\n"));
                s.success(format!("Copied {count} failed link{}", if count == 1 { "" } else { "s" }));
            }
        }));
    }
    menu.item(MenuItem::new("Clear Finished").icon(&icons::X).danger(true).on_select(|s: &mut App| {
        s.queue_menu_open = false;
        s.clear_finished();
    })).build(k, overlays)
}

fn job_card(s: &App, k: &Kit, job: &Job, first_queued_id: Option<u64>) -> El {
    let id = job.id;
    let title = job.title.clone().unwrap_or_else(|| job.url.clone());
    let mut subtitle: Vec<String> = Vec::new();
    if let Some(artist) = &job.artist {
        subtitle.push(artist.clone());
    }
    if let Some(album) = &job.album {
        subtitle.push(album.clone());
    }
    let mut meta = row().gap(FLACORA.spacing.inline).center_y().flex_wrap();
    // A resolved link shows where it came from: "Spotify → Tidal".
    if let Some((_, from)) = &job.resolved {
        meta = meta.child(service_chip(k, *from, "Link")).child(icon(&icons::ARROW_RIGHT, FLACORA.icons.badge).color(k.c.icon_faint));
    }
    if job.lookup_file.is_some() {
        let manifest = &job.extension.manifest;
        let url = crate::extensions::site_url(&manifest.description);
        let label = if manifest.can_write_tags() { "Tag Audio" } else { manifest.kind.label() };
        meta = meta.child(widgets::badge(k, label));
        if let Some(url) = url {
            let tip = format!("Open {}", manifest.description.trim());
            meta = meta.child(
                Button::new(manifest.name.clone())
                    .icon(&icons::SQUARE_ARROW_UP_RIGHT)
                    .variant(ButtonVariant::GhostMuted)
                    .control_size(ControlSize::Small)
                    .help(tip)
                    .on_press_tx(move |s: &mut App, tx| {
                        if let Err(error) = tx.open_url(&url) {
                            s.error(format!("Could not open website: {error}"));
                        }
                    })
                    .build(k)
                    .key(format!("lookup-source-{id}")),
            );
        } else {
            meta = meta.child(text(manifest.name.clone()).font(k.ty.text_xsmall).color(k.c.text_faint));
        }
    } else {
        meta = meta.child(service_chip(k, job.service, "Link"));
        let by = match &job.resolved {
            Some((resolver, _)) => format!("{resolver} · {}", job.extension.manifest.name),
            None => job.extension.manifest.name.clone(),
        };
        meta = meta.child(text(by).font(k.ty.text_xsmall).color(k.c.text_faint).truncate());
    }
    if let Some(quality) = &job.quality {
        meta = meta.child(badge(k, quality));
    }
    let mut info = col()
        .gap(FLACORA.spacing.small)
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .child(text(title).font(k.ty.body_medium).color(k.c.text_strong).truncate());
    if !subtitle.is_empty() {
        info = info.child(text(subtitle.join(" · ")).font(k.ty.small).color(k.c.text_muted).truncate());
    }
    info = info.child(meta);

    let head = row()
        .w_full()
        .gap(FLACORA.spacing.media)
        .center_y()
        .child(cover(k, s.image(job.cover.as_deref()), FLACORA.artwork.job_cover))
        .child(info)
        .child(status(k, job))
        .child(actions(k, job, first_queued_id));
    let mut body = col().gap(FLACORA.spacing.group).child(head);
    if job.is_running() {
        body = body.child(download_progress(k, job));
    }
    if job.status == Status::Choosing {
        body = body.child(choices(s, k, job));
    }
    if let Status::Failed(reason) = &job.status {
        let title = match job.extension.manifest.kind {
            crate::extensions::Kind::Downloader => "Download Failed",
            crate::extensions::Kind::Resolver => "Link Resolution Failed",
            _ if job.extension.manifest.can_write_tags() => "Tagging Failed",
            _ => "Lookup Failed",
        };
        let retry = widgets::button(k, &format!("retry-{id}"), "Try Again", Some(&icons::REFRESH_CW),
            ButtonVariant::Neutral, move |s: &mut App| s.retry_job(id));
        body = body.child(widgets::error_banner(k, format!("job-error-{id}"), title, text(reason.clone()), Some(retry)));
    }
    if job.show_log && !job.log.is_empty() {
        body = body.child(log(k, job));
    }
    card(k, format!("job-{id}"), body)
}

fn download_progress(k: &Kit, job: &Job) -> El {
    let label = job.stage.as_deref().map_or_else(|| "Working".into(), title_case_label);
    crate::astryx::progress(k, format!("progress-{}", job.id), label, job.progress)
}

/// One button per service the resolver found.
fn choices(s: &App, k: &Kit, job: &Job) -> El {
    let id = job.id;
    let resolver = job.extension.manifest.name.clone();
    let mut buttons = row().key(format!("choices-{id}")).gap(FLACORA.spacing.inline).center_y().flex_wrap();
    for (i, choice) in job.choices.iter().enumerate() {
        let label = format!("{} via {}", choice.service.name, choice.downloader_name);
        let logo = widgets::provider_logo(s, choice.service, FLACORA.artwork.provider_inline);
        let mut button = Button::new(label.clone())
            .variant(ButtonVariant::Neutral)
            .control_size(ControlSize::Regular)
            .help(format!("Download from {} with {}", choice.service.name, choice.downloader_name))
            .on_press(move |s: &mut App| s.choose(id, i));
        if let Some(logo) = logo {
            button = button.label_with(move |font, ink| {
                row().gap(FLACORA.spacing.label).center_y().child(logo.a11y_hidden()).child(text(label).font(font).color(ink))
            });
        } else {
            button = button.icon(&icons::DOWNLOAD);
        }
        buttons = buttons.child(button.compact().build(k).key(format!("choose-{id}-{i}")));
    }
    // Services the track is also on, besides the offered ones and the one the link came from.
    let mut skip: Vec<&str> = job.choices.iter().map(|c| c.service.name).collect();
    skip.extend(job.service.map(|s| s.name));
    let others: Vec<&str> = job.links.iter().map(|(service, _)| service.as_str()).filter(|s| !skip.contains(s)).collect();
    let heading = match job.batch_service {
        Some(service) if s.importing > 0 && job.choices.iter().any(|choice| choice.service == service) =>
            format!("Waiting for installation before using {}.", service.name),
        Some(service) => format!("{} is unavailable for this track. Choose a service.", service.name),
        None => format!("{resolver} found this track. Download it from:"),
    };
    let mut panel = col()
        .key(format!("choices-panel-{id}"))
        .w_full()
        .gap(FLACORA.spacing.inline)
        .padding(FLACORA.spacing.group)
        .radius(k.radius.md)
        .fill(k.c.bg_deep)
        .border(FLACORA.border.normal, k.c.border_base)
        .child(
            row()
                .gap(FLACORA.spacing.inline)
                .center_y()
                .child(icon(&icons::LINK, FLACORA.icons.inline).color(k.c.text_accent))
                .child(text(heading).key(format!("choices-heading-{id}")).font(k.ty.small_medium).color(k.c.text_strong).wrap().shrink()),
        )
        .child(buttons);
    if !others.is_empty() {
        panel = panel.child(
            text(format!("Also on {} (no installed extension downloads from there)", others.join(", ")))
                .font(k.ty.text_xsmall)
                .color(k.c.text_faint)
                .wrap(),
        );
    }
    panel
}

fn status(k: &Kit, job: &Job) -> El {
    let (glyph, label, colors) = match &job.status {
        Status::Queued => (&icons::CLOCK, "Waiting", None),
        Status::Choosing => (&icons::CHEVRONS_UP_DOWN, "Choose a Service", Some(k.c.info)),
        Status::Running => {
            return row()
                .role("status")
                .label("Downloading")
                .gap(FLACORA.spacing.label)
                .center_y()
                .child(Spinner::new().size(FLACORA.spinner.inline).build(k));
        }
        Status::Done if job.extension.manifest.can_write_tags() => (&icons::CIRCLE_CHECK, "Tagged", Some(k.c.success)),
        Status::Done => (&icons::CIRCLE_CHECK, "Saved", Some(k.c.success)),
        Status::Failed(_) => (&icons::CIRCLE_X, "Failed", Some(k.c.danger)),
        Status::Cancelled => (&icons::CIRCLE_X, "Cancelled", None),
    };
    status_badge(k, glyph, label, colors).key(format!("status-{}", job.id))
}

fn actions(k: &Kit, job: &Job, first_queued_id: Option<u64>) -> El {
    let id = job.id;
    let mut actions = row().gap(FLACORA.spacing.tight).center_y();
    match &job.status {
        Status::Done => {
            if let Some(file) = job.main_file() {
                let (play, reveal) = (file.to_owned(), file.to_owned());
                let (glyph, label) = if job.lookup_file.is_some() && !job.extension.manifest.can_write_tags() {
                    (&icons::SQUARE_ARROW_UP_RIGHT, "Open File")
                } else {
                    (&icons::PLAY, "Play")
                };
                actions = actions
                    .child(widgets::icon_button(k, &format!("play-{id}"), glyph, label, move |s: &mut App| s.open(&play)))
                    .child(widgets::icon_button(k, &format!("reveal-{id}"), &icons::FOLDER, "Show in Folder", move |s: &mut App| {
                        s.reveal(&reveal)
                    }));
            }
        }
        Status::Failed(_) => {}
        Status::Cancelled => {
            actions = actions.child(widgets::icon_button(k, &format!("retry-{id}"), &icons::REFRESH_CW, "Try Again", move |s: &mut App| {
                s.retry_job(id)
            }));
        }
        Status::Queued | Status::Running | Status::Choosing => {
            if job.status == Status::Queued && first_queued_id.is_some_and(|first| first != id) {
                actions = actions.child(widgets::icon_button(k, &format!("move-top-{id}"), &icons::ARROW_UP, "Move to Top", move |s: &mut App| {
                    s.move_job_to_top(id)
                }));
            }
            let (glyph, label) = if job.status == Status::Choosing { (&icons::X, "Skip This Track") } else { (&icons::SQUARE, "Cancel") };
            actions = actions.child(widgets::danger_icon_button(k, &format!("cancel-{id}"), glyph, label, move |s: &mut App| s.cancel_job(id))
                .disabled(job.is_running() && job.handle.is_none()));
        }
    }
    if !job.log.is_empty() {
        let glyph = if job.show_log { &icons::CHEVRON_DOWN } else { &icons::CHEVRON_RIGHT };
        actions = actions.child(
            widgets::icon_button(k, &format!("log-{id}"), glyph, "Details", move |s: &mut App| s.toggle_log(id))
                .expanded(job.show_log),
        );
    }
    if job.is_finished() {
        actions = actions.child(widgets::danger_icon_button(k, &format!("remove-{id}"), &icons::X, "Remove from List", move |s: &mut App| {
            s.remove_job(id)
        }));
    }
    actions
}

fn log(k: &Kit, job: &Job) -> El {
    let id = job.id;
    let copy = Button::new("Copy Log")
        .icon(&icons::COPY)
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Small)
        .on_press_tx(move |s: &mut App, tx: &mut Tx| {
            if let Some(job) = s.jobs.iter().find(|j| j.id == id) {
                tx.copy(job.log.join("\n"));
                s.success("Copied job log to clipboard.");
            }
        })
        .compact()
        .build(k)
        .key(format!("copy-log-{id}"));
    let header = row()
        .w_full()
        .px(FLACORA.spacing.group)
        .pt(FLACORA.spacing.small)
        .center_y()
        .justify_between()
        .child(text(format!("Log ({} lines)", job.log.len())).font(k.ty.text_xsmall).color(k.c.text_faint))
        .child(copy);

    let danger = k.c.danger.fg;
    let muted = k.c.text_muted;
    let mono = k.ty.mono;
    let lines = if job.log.len() > 24 {
        col()
            .w_full()
            .px(FLACORA.spacing.group)
            .py(FLACORA.spacing.small)
            .virtual_rows(
                job.log.len(),
                20.0,
                move |_, i| Cow::Owned(format!("log-line-{id}-{i}")),
                move |s: &App, i| {
                    let Some(line) = s.jobs.iter().find(|j| j.id == id).and_then(|j| j.log.get(i)) else {
                        return el();
                    };
                    let colour = if line.starts_with("Error:") { danger } else { muted };
                    text(line.clone()).font(mono).color(colour).wrap().pb(FLACORA.spacing.tight)
                },
            )
    } else {
        let mut list = col().w_full().px(FLACORA.spacing.group).py(FLACORA.spacing.small).gap(FLACORA.spacing.tight);
        for line in &job.log {
            let colour = if line.starts_with("Error:") { danger } else { muted };
            list = list.child(text(line.clone()).font(mono).color(colour).wrap());
        }
        list
    };
    let viewport = lines
        .key(format!("log-scroll-{id}"))
        .scroll(ScrollSpec { stick_bottom: true })
        .max_h(FLACORA.queue.log_max_height)
        .scroll_bar_styled(widgets::scroll_bar_style(k));
    col()
        .key(format!("log-view-{id}"))
        .w_full()
        .radius(k.radius.xs)
        .fill(k.c.bg_deep)
        .border(FLACORA.border.normal, k.c.border_base)
        .child(header)
        .child(viewport)
}
