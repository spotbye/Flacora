//! The window: a sidebar with the five pages, banners, and the drop overlay.

mod catalog;
mod downloads;
mod extensions;
mod library;
mod settings;
use crate::astryx::theme;
mod widgets;

pub use theme::FONTS;
pub const MIN_WINDOW_SIZE: (f32, f32) = (theme::FLACORA.window.min_width, theme::FLACORA.window.min_height);

use crate::app::{App, Notification, NotificationKind, Page};
use crate::{icons, store};
use rune::{Chord, ChordKey, ChordMod, Cursor, Cx, El, Icon, Image, ImageCache, SurfaceFit, col, el, icon, row, text};
use crate::astryx::{Divider, Sidebar, SidebarRow, Toast, ToastRegion};
use theme::{Kit, FLACORA};

pub const SIDEBAR: f32 = FLACORA.sidebar.width;

struct Project {
    id: &'static str,
    name: &'static str,
    website: &'static str,
    status: &'static str,
    logo: &'static [u8],
}

const PROJECTS: [Project; 2] = [
    Project {
        id: "spotbye",
        name: "SpotBye",
        website: "https://spotbye.vip/",
        status: "https://status.spotbye.vip/",
        logo: include_bytes!("../../assets/projects/spotbye.svg"),
    },
    Project {
        id: "stream-hoarder",
        name: "Stream Hoarder",
        website: "https://streamhoarder.app/",
        status: "https://status.streamhoarder.app/",
        logo: include_bytes!("../../assets/projects/stream-hoarder.svg"),
    },
];

pub fn project_logos(cache: &ImageCache) -> [Image; 2] {
    PROJECTS.map(|project| cache.svg(project.logo, FLACORA.artwork.project_logo, FLACORA.artwork.project_scale))
}

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let dark = s.dark(cx.system_dark());
    let k = theme::kit(s.settings.theme_preset, dark);
    cx.foreground(k.c.text_base);
    // Keep the region mounted while its last notification fades out.
    let mut overlays = vec![notifications(s, k)];
    let page = match s.page {
        Page::Downloads => downloads::page(s, k, &mut overlays),
        Page::Catalog => catalog::page(s, k),
        Page::Library => library::page(s, k),
        Page::Extensions => extensions::page(s, k, &mut overlays),
        Page::Settings => settings::page(s, k, dark),
    };
    let content = col().key("content").grow(1.0).shrink().min_w(0.0).h_full().items_center().child(page);
    // Extension packages can be dropped anywhere on the window.
    let base = row()
        .key("window")
        .w_full()
        .h_full()
        .fill(k.c.page)
        .child(sidebar(s, k))
        .child(Divider::vertical().build(k))
        .child(content)
        .on_drop_files(|s: &mut App, files, _| s.install_files(files))
        .accepts_extensions(&store::PACKAGE_EXTENSIONS)
        .on_drop_hover(|s: &mut App, over, _| s.drop_over = over)
        .on_drop_rejected(|s: &mut App, files, _| s.reject_files(files));
    let empty_extensions = s.page == Page::Extensions && s.loaded && s.installed.is_empty();
    if s.drop_over && !empty_extensions {
        overlays.push(drop_overlay(k));
    }
    crate::astryx::root(base, overlays)
}

fn sidebar(s: &App, k: &Kit) -> El {
    // The PNG has a transparent margin around its rounded square, so it draws a little larger.
    let logo = match s.logo.as_ref().filter(|l| l.error().is_none()) {
        Some(image) => el().key("logo").size(FLACORA.artwork.app_logo, FLACORA.artwork.app_logo).pixels_fit(image.pixels(), rune::SurfaceFit::Fit),
        None => el()
            .key("logo")
            .size(FLACORA.artwork.app_fallback, FLACORA.artwork.app_fallback)
            .radius(k.radius.md)
            .fill(k.c.bg_contrast)
            .center()
            .child(icon(&icons::DISC, FLACORA.artwork.app_glyph).color(k.c.text_contrast)),
    };
    let brand = row()
        .key("brand")
        .gap(FLACORA.spacing.row)
        .center_y()
        .px(FLACORA.sidebar.brand_padding)
        .child(logo)
        .child(
            col()
                .child(text("Flacora").font(k.subheading).color(k.c.text_strong))
                .child(
                    row()
                        .key("sidebar-version")
                        .role("button")
                        .label(concat!("Version ", env!("CARGO_PKG_VERSION"), ", check for updates"))
                        .help(if s.checking_app_update {
                            "Checking for Flacora updates…"
                        } else {
                            "Click to check for Flacora updates"
                        })
                        .cursor(Cursor::Pointer)
                        .radius(k.radius.xs)
                        .on_press(|s: &mut App| s.recheck_app_update())
                        .child(
                            text(if s.checking_app_update {
                                "Checking…"
                            } else {
                                concat!("v", env!("CARGO_PKG_VERSION"))
                            })
                            .font(k.ty.text_xsmall)
                            .color(k.c.text_faint)
                            .a11y_hidden(),
                        ),
                ),
        );
    let active = s.active_jobs();
    let extensions = s.installed.len();
    let items: [(Page, &'static Icon, &str, Option<String>); 5] = [
        (Page::Downloads, &icons::DOWNLOAD, "Downloads", (active > 0).then(|| active.to_string())),
        (Page::Catalog, &icons::SEARCH, "Catalog", None),
        (Page::Library, &icons::LIBRARY, "Library", (!s.history.is_empty()).then(|| s.history.len().to_string())),
        (Page::Extensions, &icons::PUZZLE, "Extensions", s.loaded.then(|| extensions.to_string())),
        (Page::Settings, &icons::SETTINGS, "Settings", None),
    ];
    let mut sidebar = Sidebar::new().label("Pages").header(brand);
    for (index, (page, glyph, label, badge)) in items.into_iter().enumerate() {
        let selected = s.page == page;
        let digit = char::from_digit((index + 1) as u32, 10).expect("sidebar index digit");
        let shortcut = Chord::new(ChordKey::Char(digit)).with(ChordMod::Primary).label();
        let mut item = SidebarRow::new(label)
            .icon(glyph)
            .selected(selected)
            .on_select(move |s: &mut App, _| {
                s.page = page;
                s.dismiss_page_overlays();
                if page == Page::Library {
                    s.refresh_library_files();
                }
            });
        if let Some(badge) = badge {
            item = item.badge(badge);
        }
        let mut item = item.build(k).key(format!("nav-{}", label.to_lowercase())).shortcut_hint(shortcut);
        if page == Page::Downloads && active > 0 {
            item = item.label(format!("{label}, {active} active"));
        } else if page == Page::Library && !s.history.is_empty() {
            item = item.label(format!("{label}, {} downloads", s.history.len()));
        } else if page == Page::Extensions && s.loaded {
            item = item.label(format!("{label}, {extensions} installed"));
        }
        sidebar = sidebar.child(item);
    }
    let coffee = SidebarRow::new("Buy Me a Coffee")
        .icon(&icons::COFFEE)
        .on_select(|s, tx| open_footer_url(s, tx, "https://afkarxyz.gumroad.com/coffee"))
        .build(k).key("buy-me-a-coffee").role("link");
    let mut footer = col().w_full().gap(FLACORA.spacing.tight).pb(FLACORA.spacing.inline);
    if let Some(release) = &s.available_update {
        let url = release.url.clone();
        footer = footer.child(row().h(FLACORA.sidebar.update_height).center().radius(FLACORA.sidebar.update_radius)
            .key("sidebar-update").label("Update Available")
            .self_start().ml(FLACORA.spacing.label).mb(FLACORA.spacing.small)
            .role("link").focusable(true)
            .help(format!("Open Flacora v{} on GitHub", release.version))
            .cursor(Cursor::Pointer)
            .hover(k.c.hover)
            .focus_ring(theme::FOCUS_RING_WIDTH, theme::FOCUS_RING_OFFSET, k.c.border_focus)
            .on_press_tx(move |s, tx| open_footer_url(s, tx, url.as_str()))
            .child(widgets::status_badge(k, &icons::CIRCLE_CHECK, "Update Available", Some(k.c.success)).a11y_hidden()));
    }
    let footer = footer.child(coffee).child(project_shortcuts(s, k));
    sidebar.footer(footer).build(k)
}

fn project_shortcuts(s: &App, k: &Kit) -> El {
    let links = PROJECTS.iter().enumerate().map(|(i, project)| {
        let mut logo = el().size(FLACORA.artwork.project_logo, FLACORA.artwork.project_logo).a11y_hidden();
        if let Some(images) = &s.project_logos {
            logo = logo.pixels_fit(images[i].pixels(), SurfaceFit::Fit);
        }
        let website = footer_link(k, format!("project-{}", project.id), project.name.into(), project.website)
            .grow(1.0)
            .min_w(0.0)
            .pr(FLACORA.spacing.small)
            .gap(FLACORA.spacing.inline)
            .child(logo)
            .child(text(project.name).font(k.ty.ui).color(k.c.text_base).truncate().a11y_hidden());
        let status = footer_link(k, format!("project-{}-status", project.id), format!("{} Server Status", project.name), project.status)
            .gap(FLACORA.spacing.small)
            .child(icon(&icons::ACTIVITY, FLACORA.icons.small).color(k.c.icon_muted).a11y_hidden())
            .child(text("Status").font(k.ty.small).color(k.c.text_muted).a11y_hidden());
        row().w_full().gap(FLACORA.spacing.small).center_y().child(website).child(status)
    });
    col().key("project-shortcuts").role("group").label("Projects").w_full().gap(FLACORA.spacing.tight).children(links)
}

fn footer_link(k: &Kit, key: String, label: String, url: &'static str) -> El {
    crate::astryx::inline_link(k, key, label)
        .on_press_tx(move |s, tx| open_footer_url(s, tx, url))
}

fn open_footer_url(s: &mut App, tx: &mut rune::Tx, url: &str) {
    if let Err(error) = tx.open_url(url) {
        s.error(format!("Could not open website: {error}"));
    }
}

fn notifications(s: &App, k: &Kit) -> El {
    let mut region = ToastRegion::new();
    if let Some(notice) = &s.notice {
        region = region.toast(notification(
            k,
            "notice",
            notice,
            false,
            |s| {
                s.notice = None;
                s.notice_hovered = false;
            },
            |s, inside| s.notice_hovered = inside,
        ));
    }
    if let Some(info) = &s.info {
        region = region.toast(notification(
            k,
            "info",
            info,
            s.undo.is_some(),
            |s| {
                s.info_hovered = false;
                s.dismiss_info();
            },
            |s, inside| s.info_hovered = inside,
        ));
    }
    // Keep the overlay's origin fixed on the bottom-left of the content pane so right-aligned card actions stay clear.
    region.build(k).pin(Some(SIDEBAR + FLACORA.overlay.toast_margin), Some(0.0), None, Some(FLACORA.overlay.toast_margin)).w(FLACORA.overlay.toast_width).justify_end()
}

fn notification(
    k: &Kit,
    key: &str,
    note: &Notification,
    has_undo: bool,
    dismiss: fn(&mut App),
    on_hover: fn(&mut App, bool),
) -> El {
    let tone = match note.kind {
        NotificationKind::Success => crate::astryx::Tone::Success,
        NotificationKind::Warning => crate::astryx::Tone::Warning,
        NotificationKind::Error => crate::astryx::Tone::Error,
    };
    let mut toast = Toast::new(k, key, tone, &note.message).on_close(dismiss);
    if has_undo {
        toast = toast.action(k, "Undo", |s: &mut App, _| s.undo_last());
    }
    toast.build(k).on_hover_change(move |s: &mut App, inside, _| on_hover(s, inside))
}

fn drop_overlay(k: &Kit) -> El {
    let card = col()
        .key("drop-card")
        .w_full()
        .h_full()
        .gap(FLACORA.spacing.row)
        .center()
        .radius(k.card_radius)
        .border(FLACORA.border.emphasis, k.c.border_focus)
        .border_dash(FLACORA.overlay.dash_length, FLACORA.overlay.dash_gap)
        .fill(k.colors.drop_overlay_surface)
        .child(
            el().size(FLACORA.artwork.drop_frame, FLACORA.artwork.drop_frame)
                .radius(FLACORA.artwork.round_radius)
                .fill(k.c.layer_02)
                .center()
                .child(icon(&icons::FILE_UP, FLACORA.artwork.drop_glyph).color(k.c.icon_base)),
        )
        .child(text("Drop to Install").font(k.heading).color(k.c.text_strong))
        .child(text(format!("Flacora Extension Packages (.{})", store::PACKAGE)).font(k.ty.small).color(k.c.text_muted));
    // Absolute, so it covers the window instead of being laid out beside it.
    el().key("drop-overlay").absolute(0.0, 0.0).w_full().h_full().padding(FLACORA.overlay.drop_inset).fill(k.c.scrim).child(card)
}

#[cfg(test)]
mod tests;
