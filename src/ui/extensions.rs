//! Locally installed extensions and the package drop target.

use super::theme::FLACORA;
use super::theme::{FOCUS_RING_OFFSET, FOCUS_RING_WIDTH, Kit};
use super::widgets::{self, badge, badge_with_icon, card, section, service_chip, status_badge};
use crate::app::{App, ConfirmAction};
use crate::extensions::Kind;
use crate::extension_updates::{self, CheckState};
use crate::icons;
use crate::services;
use crate::store::{self, ExtensionStatus, Installed};
use rune::{Cursor, El, GrowthSpec, Link, LinkPaint, Spring, Tx, col, el, icon, row, text};
use crate::astryx::{Button, ButtonVariant, Divider, Spinner};

const CATEGORIES: [(Kind, &str); 4] = [
    (Kind::Downloader, "downloaders"),
    (Kind::Resolver, "resolvers"),
    (Kind::Metadata, "metadata"),
    (Kind::Lyrics, "lyrics"),
];
const LIST_SPRING: Spring = Spring::new(FLACORA.motion.extensions_response, FLACORA.motion.extensions_bounce);
const EXTENSIONS_URL: &str = "https://rentry.org/flacora";

fn select_filter(s: &mut App, tx: &mut Tx, filter: Option<Kind>) {
    if s.extension_filter != filter {
        s.extension_filter = filter;
        tx.scroll_to_with("extension-list", 0.0, 0.0, LIST_SPRING);
    }
}

pub fn page(s: &App, k: &Kit, overlays: &mut Vec<El>) -> El {
    let mut actions = Vec::new();
    if s.loaded && !s.installed.is_empty() {
        actions.push(widgets::icon_button(k, "check-extension-updates", &icons::REFRESH_CW,
            "Check Extension Updates", App::check_extension_updates)
            .disabled(s.extension_update_check == CheckState::Checking));
        let checking = s.any_checking();
        let label = if checking { "Checking Status…" } else { "Check Status" };
        actions.push(widgets::button(k, "check-extension-status", label, Some(&icons::REFRESH_CW), ButtonVariant::Outline,
            App::check_all).disabled(checking || s.importing > 0));
        actions.push(widgets::button(k, "remove-all-extensions", "Remove All", Some(&icons::TRASH_2), ButtonVariant::Outline, |s: &mut App| {
            if s.removing.is_empty() {
                s.confirm_action = Some(ConfirmAction::RemoveAllExtensions);
            }
        }).disabled(!s.removing.is_empty()));
        let label = if s.importing > 0 { "Installing Packages…" } else { "Install Extension" };
        actions.push(Button::new(label)
            .variant(ButtonVariant::Submit)
            .icon(&icons::FILE_UP)
            .help("Browse for Extension Packages")
            .disabled(s.browsing_packages)
            .on_press_tx(App::browse_packages)
            .action().build(k).key("install-extension"));
    }
    if s.confirm_action == Some(ConfirmAction::RemoveAllExtensions) {
        overlays.push(remove_all_dialog(s, k));
    }
    let head = widgets::page_header_with_subtitle(k, "Extensions", update_summary(s, k), actions);
    let mut page = widgets::page("extensions-page").child(head);
    if s.loaded && s.installed.is_empty() {
        return page.child(col().w_full().grow(1.0).min_h(0.0).center().child(drop_zone(s, k)));
    }

    let mut list = col().key("extension-cards").growth(GrowthSpec::default());
    let mut filter_bar = None;
    if !s.loaded {
        list = list.child(
            row()
                .key("extensions-loading")
                .role("status")
                .gap(FLACORA.spacing.inline)
                .center_y()
                .child(Spinner::new().size(FLACORA.spinner.inline).build(k))
                .child(text("Loading Installed Extensions…").font(k.ty.body).color(k.c.text_muted)),
        );
    } else {
        let present_categories: Vec<_> = CATEGORIES
            .iter()
            .filter_map(|&(kind, key)| {
                let count = s.installed.iter().filter(|i| i.manifest.kind == kind).count();
                (count > 0).then_some((kind, key, count))
            })
            .collect();
        let active_filter = s.extension_filter.filter(|kind| present_categories.iter().any(|(k, _, _)| k == kind));
        if present_categories.len() > 1 {
            let total = present_categories.len() + 1;
            let mut chips = widgets::filter_group("extension-filters", "Extension Type")
                .child(widgets::filter_button(
                    k, "ext-filter-all".into(), format!("All ({})", s.installed.len()),
                    active_filter.is_none(), None, |s, tx| select_filter(s, tx, None),
                ).position_in_set(1, total));
            for (idx, &(kind, key, count)) in present_categories.iter().enumerate() {
                let selected = active_filter == Some(kind);
                chips = chips.child(widgets::filter_button(
                    k, format!("ext-filter-{key}"), format!("{} ({count})", kind.label()), selected, None,
                    move |s, tx| select_filter(s, tx, Some(kind)),
                ).position_in_set(idx + 2, total));
            }
            filter_bar = Some(chips);
        }
        let mut rows = Vec::new();
        for (kind, key) in CATEGORIES {
            if active_filter.is_some_and(|filter| filter != kind) {
                continue;
            }
            let mut installed: Vec<_> = s.installed.iter().filter(|i| i.manifest.kind == kind).collect();
            if installed.is_empty() {
                continue;
            }
            installed.sort_by_cached_key(|i| i.manifest.name.to_lowercase());
            rows.push(section(k, kind.label(), Some(installed.len())).key(format!("extension-category-{key}")));
            rows.push(el().key(format!("gap-after-category-{key}")).h(FLACORA.spacing.row).a11y_hidden());
            for extension in installed {
                rows.push(extension_card(s, k, extension));
                rows.push(el().key(format!("gap-after-card-{}", extension.manifest.id)).h(FLACORA.spacing.row).a11y_hidden());
            }
        }
        // Grow the gaps with the rows so they do not snap shut at the end of a collapse.
        rows.pop();
        list = list.children(rows.into_iter().map(|row| row.fold_glide(LIST_SPRING).fold_feather(FLACORA.motion.extensions_feather)));
    }
    if let Some(chips) = filter_bar {
        page = page.child(chips);
    }
    page.child(widgets::page_scroll(k, col().key("extension-list").child(list)))
}

fn remove_all_dialog(s: &App, k: &Kit) -> El {
    let count = s.installed.len();
    let blocker = s.remove_all_extensions_blocker();
    let extensions = format!("{count} installed extension{}", if count == 1 { "" } else { "s" });
    let detail = if let Some(reason) = blocker {
        format!("{extensions}. {reason}")
    } else {
        format!("This will uninstall {extensions}. You can reinstall them from their .fext packages.")
    };
    let keep = widgets::button(k, "keep-extensions", "Keep Extensions", None, ButtonVariant::Neutral,
        |s: &mut App| s.confirm_action = None);
    let remove = widgets::button(k, "confirm-remove-all-extensions", "Remove All", Some(&icons::TRASH_2), ButtonVariant::Danger,
        App::remove_all_extensions).disabled(count == 0 || blocker.is_some());
    widgets::confirmation_dialog(k, "remove-all-extensions", "Remove all extensions?", detail,
        "remove-all-extensions", keep, remove)
}

fn update_summary(s: &App, k: &Kit) -> El {
    let mut summary = col().gap(FLACORA.spacing.small).child(download_link(k));
    let message = match s.extension_update_check {
        CheckState::Idle => return summary,
        CheckState::Checking => "Checking extension versions…".to_owned(),
        CheckState::Failed => if s.extension_versions.is_empty() {
            "Could not check updates. Try again.".to_owned()
        } else { "Could not check updates. Try again. Previous results are kept.".to_owned() },
        CheckState::Ready => {
            let count = s.installed.iter().filter(|i| extension_updates::newer(&s.extension_versions, &i.manifest.id, &i.manifest.version).is_some()).count();
            if count > 0 { format!("{count} update{} available. Download packages manually.", if count == 1 { "" } else { "s" }) }
            else { "Version information checked.".to_owned() }
        }
    };
    summary = summary.child(text(message).key("extension-update-summary").role("status")
        .font(k.ty.small).color(if s.extension_update_check == CheckState::Failed { k.c.warning.fg } else { k.c.text_muted }).wrap());
    summary
}

fn download_link(k: &Kit) -> El {
    let label = "Get Extensions on Rentry";
    let link = Link::new(0..label.len(), EXTENSIONS_URL)
        .hover(LinkPaint::new().underline(k.c.info.fg));
    text(label).key("download-extensions").self_start().font(k.ty.body_medium).color(k.c.info.fg)
        .link(link)
        .on_link(|s: &mut App, url, tx| {
            if let Err(error) = tx.open_url(url) {
                s.error(format!("Could not open Rentry: {error}"));
            }
        })
}

fn drop_zone(s: &App, k: &Kit) -> El {
    let active = s.drop_over;
    let installing = s.importing > 0 && !active;
    let label = if installing {
        "Installing Packages…".to_owned()
    } else if active {
        "Drop to Install".to_owned()
    } else {
        format!("Drop .{} Packages", store::PACKAGE)
    };

    let accent = if active { k.c.border_focus } else { k.c.text_strong };
    let hint_color = k.drop_hint;
    let symbol = if installing {
        Spinner::new().size(FLACORA.spinner.large).build(k)
    } else {
        icon(&icons::FILE_UP, FLACORA.artwork.drop_glyph).color(accent)
    };
    let mut message = col().center().gap(FLACORA.spacing.label)
        .child(text(label).role(if installing { "status" } else { "heading" }).font(k.subheading).color(accent));
    if !installing {
        message = message.child(
            text("Drop them anywhere in this window, or click here to browse.").font(k.ty.body).color(hint_color),
        );
    }

    col().w_full().max_w(FLACORA.overlay.extension_drop_width).h(FLACORA.overlay.extension_drop_height).center().gap(FLACORA.spacing.section)
        .child(el().size(FLACORA.artwork.drop_frame, FLACORA.artwork.drop_frame).center().radius(FLACORA.artwork.round_radius).fill(if active { k.c.layer_04 } else { k.c.layer_02 })
            .child(symbol).a11y_hidden())
        .child(message)
        .key("drop-zone")
        .role("button")
        .label("Browse for Extension Packages")
        .focusable(true)
        .focus_ring(FOCUS_RING_WIDTH, FOCUS_RING_OFFSET, k.c.border_focus)
        .cursor(Cursor::Pointer)
        .radius(k.radius.md)
        .fill(if active { k.colors.drop_target_active } else { k.c.bg_base })
        .hover(k.c.hover)
        .border(if active { FLACORA.border.emphasis } else { FLACORA.border.normal }, if active { k.c.border_focus } else { k.c.border_strong })
        .border_dash(FLACORA.overlay.extension_dash_length, FLACORA.overlay.dash_gap)
        .on_press_tx(App::browse_packages)
}

fn extension_card(s: &App, k: &Kit, installed: &Installed) -> El {
    let manifest = &installed.manifest;
    let id = &manifest.id;
    let kind = manifest.kind;
    let website = manifest
        .description
        .trim()
        .strip_prefix("https://")
        .or_else(|| manifest.description.trim().strip_prefix("http://"))
        .unwrap_or(manifest.description.trim());
    let site_url = crate::extensions::site_url(&manifest.description);

    let mut title = row().gap(FLACORA.spacing.label).center_y().flex_wrap()
        .child(text(manifest.name.clone()).font(k.ty.body_medium).color(k.c.text_strong))
        .child(badge(k, format!("v{}", manifest.version)));
    if let Some(latest) = extension_updates::newer(&s.extension_versions, id, &manifest.version) {
        let update_text = format!("v{latest} available");
        let link = Link::new(0..update_text.len(), EXTENSIONS_URL)
            .hover(LinkPaint::new().underline(k.c.info.fg));
        title = title.child(
            text(update_text)
                .key(format!("extension-update-{id}"))
                .role("status")
                .label(format!("Update available: version {latest}. Installed version {}. Download the package manually.", manifest.version))
                .help("Open Rentry to download the updated .fext package, then drop it into Flacora.")
                .font(k.ty.small_medium)
                .color(k.c.info.fg)
                .link(link)
                .on_link(|s: &mut App, url, tx| {
                    if let Err(error) = tx.open_url(url) {
                        s.error(format!("Could not open Rentry: {error}"));
                    }
                }),
        );
    }
    let mut info = col().items_start().gap(FLACORA.spacing.label).grow(1.0).shrink().min_w(0.0).child(title);
    if !website.is_empty() {
        if let Some(href) = site_url {
            let link = Link::new(0..website.len(), href)
                .hover(LinkPaint::new().color(k.c.text_base).underline(k.c.text_base));
            info = info.child(
                text(website.to_owned()).key(format!("site-{id}"))
                    .font(k.ty.text_small).color(k.c.text_muted).truncate()
                    .link(link)
                    .on_link(|s: &mut App, url, tx| {
                        if let Err(error) = tx.open_url(url) {
                            s.error(format!("Could not open website: {error}"));
                        }
                    }),
            );
        } else {
            info = info.child(
                text(website.to_owned())
                    .key(format!("desc-{id}"))
                    .font(k.ty.text_small)
                    .color(k.c.text_muted)
                    .wrap(),
            );
        }
    }
    if !kind.is_lookup() {
        let service_names = s.services_of(id, &manifest.services);
        if !service_names.is_empty() {
            info = info.child(row().key(format!("services-{id}")).gap(FLACORA.spacing.label).center_y().flex_wrap()
                .children(service_names.iter().map(|service| service_badge(s, k, service))));
        }
    }
    let header = row().key("header").w_full().gap(FLACORA.spacing.media).center_y()
        .child(kind_tile(k, id, kind))
        .child(info)
        .child(card_actions(s, k, installed));
    let mut body = col().w_full().child(header);
    if let Some((reason, color)) = status_issue(s, k, id) {
        body = body.child(
            text(reason)
                .key(format!("status-detail-{id}"))
                .w_full()
                .pt(FLACORA.spacing.inline)
                .font(k.ty.text_xsmall)
                .color(color)
                .wrap(),
        );
    }
    if !manifest.options.is_empty() && s.extension_settings.as_deref() == Some(id) {
        let mut options = col().key(format!("extension-options-{id}")).w_full().gap(FLACORA.spacing.inline);
        for option in &manifest.options {
            options = options.child(
                row().w_full().gap(FLACORA.spacing.group).center_y().flex_wrap()
                    .child(text(widgets::title_case_label(&option.label)).font(k.ty.small_medium).color(k.c.text_strong).grow(1.0))
                    .child(widgets::extension_option(k, s, id, option, &format!("setting-{id}-{}", option.key))),
            );
        }
        body = body.child(col().key("settings-panel").w_full().pt(FLACORA.spacing.group).gap(FLACORA.spacing.group)
            .child(Divider::horizontal().build(k))
            .child(options));
    }
    card(k, format!("ext-{id}"), body)
}

fn kind_tile(k: &Kit, id: &str, kind: Kind) -> El {
    let (glyph, tile_index) = match kind {
        Kind::Downloader => (&icons::CLOUD_DOWNLOAD, 0),
        Kind::Resolver => (&icons::SHARE_2, 1),
        Kind::Lyrics => (&icons::MIC_VOCAL, 2),
        Kind::Metadata => (&icons::TAG, 3),
    };
    let [fill, ink] = k.colors.kind_tile(tile_index);
    el().key(format!("kind-{id}"))
        .role("img")
        .label(kind.label())
        .help(kind.label())
        .size(FLACORA.artwork.kind_tile, FLACORA.artwork.kind_tile)
        .radius(k.radius.md)
        .fill(fill)
        .center()
        .child(icon(glyph, FLACORA.artwork.kind_glyph).key(glyph.name).color(ink).a11y_hidden())
}

fn service_badge(s: &App, k: &Kit, name: &str) -> El {
    let service = services::by_name(name);
    if let Some(service) = service
        && let Some(logo) = widgets::provider_logo(s, service, FLACORA.artwork.provider_inline)
    {
        return badge_with_icon(k, service.name, logo);
    }
    service_chip(k, service, name)
}

fn card_actions(s: &App, k: &Kit, installed: &Installed) -> El {
    let id = &installed.manifest.id;
    if s.removing.contains(id) {
        return row().gap(FLACORA.spacing.label).center_y()
            .child(Spinner::new().size(FLACORA.spinner.inline).build(k))
            .child(text("Removing…").font(k.ty.small).color(k.c.text_muted));
    }
    let confirming = s.confirm_action.as_ref() == Some(&ConfirmAction::RemoveExtension(id.clone()));
    let remove_id = id.to_owned();
    let disabled = s.importing > 0 || s.running_extension(id) || s.status_command_running(installed);
    let remove = if confirming {
        row()
            .gap(FLACORA.spacing.small)
            .center_y()
            .child(widgets::button(k, &format!("cancel-remove-{id}"), "Cancel", None, ButtonVariant::Ghost, |s: &mut App| {
                s.confirm_action = None;
            }))
            .child(
                widgets::button(k, &format!("remove-{id}"), "Confirm", Some(&icons::TRASH_2), ButtonVariant::Danger, move |s: &mut App| {
                    s.confirm_action = None;
                    s.remove(&remove_id);
                })
                .disabled(disabled),
            )
    } else {
        widgets::danger_icon_button(k, &format!("remove-{id}"), &icons::TRASH_2, "Remove", move |s: &mut App| {
            s.confirm_action = Some(ConfirmAction::RemoveExtension(remove_id.clone()));
        })
        .disabled(disabled)
    };
    let mut actions = row().gap(FLACORA.spacing.inline).center_y();
    if !installed.manifest.options.is_empty() {
        let id = id.clone();
        let expanded = s.extension_settings.as_deref() == Some(&id);
        let mut gear = widgets::icon_button(k, &format!("configure-{id}"), &icons::SETTINGS, "Extension Settings", move |s: &mut App| {
            s.extension_settings = (s.extension_settings.as_deref() != Some(&id)).then(|| id.clone());
        })
        .expanded(expanded);
        if expanded {
            gear = gear.fill(k.c.layer_04);
        }
        actions = actions.child(gear);
    }
    let data_id = id.clone();
    actions = actions.child(widgets::icon_button(
        k,
        &format!("data-{id}"),
        &icons::FOLDER,
        "Open Data Folder",
        move |s: &mut App| s.open_extension_data(&data_id),
    ));
    actions
        .child(online_status(s, k, id))
        .child(remove)
}

fn status_issue(s: &App, k: &Kit, id: &str) -> Option<(String, rune::Color)> {
    if s.is_checking(id) {
        return None;
    }
    match s.extension_status.get(id) {
        Some(ExtensionStatus::Offline(reason)) if !reason.trim().is_empty() => Some((reason.clone(), k.c.danger.fg)),
        Some(ExtensionStatus::Unknown(reason)) if !reason.trim().is_empty() => Some((reason.clone(), k.c.warning.fg)),
        _ => None,
    }
}

fn online_status(s: &App, k: &Kit, id: &str) -> El {
    let checking = s.is_checking(id);
    let (glyph, label, colors, detail) = if checking {
        (&icons::CLOCK, "Checking…", None, "Checking online status.")
    } else {
        match s.extension_status.get(id) {
            Some(ExtensionStatus::Online) => (&icons::CIRCLE_CHECK, "Online", Some(k.c.success), "The status endpoint responded successfully."),
            Some(ExtensionStatus::Degraded) => (&icons::TRIANGLE_ALERT, "Degraded", Some(k.c.warning), "The endpoint reports degraded service."),
            Some(ExtensionStatus::Offline(reason)) => (&icons::CIRCLE_X, "Offline", Some(k.c.danger), reason.as_str()),
            Some(ExtensionStatus::Unknown(reason)) => (&icons::TRIANGLE_ALERT, "Status Unknown", Some(k.c.warning), reason.as_str()),
            Some(ExtensionStatus::Services(services)) if services.is_empty() =>
                (&icons::CIRCLE_X, "Offline", Some(k.c.danger), "The extension reported no online services."),
            Some(ExtensionStatus::Services(_)) => (&icons::CIRCLE_CHECK, "Online", Some(k.c.success), "The extension reported online services."),
            None => (&icons::CLOCK, "Not Checked", None, "Online status has not been checked yet."),
        }
    };
    let can_recheck = !checking && s.importing == 0 && !s.removing.contains(id);
    let help = if can_recheck { format!("{detail} Click to check again.") } else { detail.to_owned() };
    let key = format!("online-status-{id}");
    let badge = status_badge(k, glyph, label, colors);
    if can_recheck {
        let id = id.to_owned();
        return row().h(FLACORA.badge.interactive_height).center().radius(FLACORA.badge.status_radius)
            .role("button")
            .label(label)
            .key(key).help(help)
            .focusable(true)
            .focus_ring(FOCUS_RING_WIDTH, FOCUS_RING_OFFSET, k.c.border_focus)
            .cursor(Cursor::Pointer)
            .hover(k.c.hover)
            .press(k.c.pressed)
            .on_press(move |s: &mut App| s.check_extension(&id))
            .child(badge.a11y_hidden());
    }
    badge.key(key).help(help)
}
