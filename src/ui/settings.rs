//! Settings: download folder, appearance, default extensions, helpers, storage.

use super::theme::FLACORA;
use super::theme::Kit;
use super::widgets::{self, card, section};
use crate::app::{App, RESOLVER_KEY};
use crate::extensions::Kind;
use crate::icons;
use crate::output::{Filename, Folders, Layout};
use crate::services::SERVICES;
use crate::store::{self, Theme, ThemePreset};
use rune::{Color, El, FileDialogOptions, Icon, Tx, col, el, icon, row, text};
use crate::astryx::{Button, ButtonVariant, ControlSize, Divider, SegmentedControl};

pub fn page(s: &App, k: &Kit, dark: bool) -> El {
    let head = widgets::page_header(k, "Settings", "Where downloads go, how Flacora looks, and which extension handles each service.", vec![]);
    let mut downloads = vec![folder_row(s, k), filename_row(s, k), folders_row(s, k), preview_row(s, k)];
    if s.download_tag_writers().next().is_some() {
        downloads.push(auto_tag_row(s, k));
    }
    let body = col()
        .key("settings-list")
        .gap(FLACORA.spacing.row)
        .child(section(k, "Downloads", None))
        .child(group(k, "group-downloads", downloads))
        .child(section(k, "Appearance", None))
        .child(group(k, "group-appearance", vec![theme_preset_row(s, k, dark), theme_row(s, k)]))
        .child(section(k, "Default Extension", None))
        .child(group(k, "group-defaults", default_rows(s, k)))
        .child(
            row()
                .w_full()
                .center_y()
                .justify_between()
                .child(section(k, "Helpers", None))
                .child(widgets::icon_button(k, "refresh-tools", &icons::REFRESH_CW, "Refresh Helpers", |s: &mut App| s.refresh_tools())),
        )
        .child(group(k, "group-helpers", vec![ffmpeg_row(s, k), browser_row(s, k)]))
        .child(section(k, "Queue", None))
        .child(group(k, "group-queue", vec![concurrency_row(s, k)]))
        .child(section(k, "Storage", None))
        .child(group(k, "group-storage", vec![data_row(s, k), cover_cache_row(s, k), history_row(s, k)]));
    widgets::page("settings-page").child(head).child(widgets::page_scroll(k, body))
}

fn group(k: &Kit, key: &str, rows: Vec<El>) -> El {
    let mut body = col();
    let last = rows.len().saturating_sub(1);
    for (i, row) in rows.into_iter().enumerate() {
        body = body.child(row);
        if i < last {
            body = body.child(Divider::horizontal().build(k));
        }
    }
    card(k, key, body)
}

/// A setting: title and explanation on the left, its control on the right.
fn setting(k: &Kit, key: &str, title: &str, detail: El, control: El) -> El {
    let label = col().gap(FLACORA.spacing.text)
        .child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_strong))
        .child(detail);
    setting_row(key, label, control)
}

fn setting_row(key: &str, label: El, control: El) -> El {
    row()
        .key(key.to_owned())
        .w_full()
        .px(FLACORA.spacing.small)
        .py(FLACORA.spacing.row)
        .gap(FLACORA.spacing.section)
        .center_y()
        .flex_wrap()
        .child(label.w(FLACORA.settings.label_width).grow(1.0).shrink().min_w(FLACORA.settings.label_width))
        .child(control)
}

fn muted(k: &Kit, t: impl Into<String>) -> El {
    text(t.into()).font(k.ty.small).color(k.c.text_muted).wrap()
}

fn folder_row(s: &App, k: &Kit) -> El {
    let change = Button::new("Change…")
        .icon(&icons::FOLDER)
        .variant(ButtonVariant::Neutral)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            let options = FileDialogOptions { title: "Save Downloads To".into(), directory: Some(s.settings.output.clone()), ..Default::default() };
            tx.open_folder(options, |s: &mut App, result, _: &mut Tx| match result {
                Ok(Some(path)) => s.set_output(path),
                Ok(None) => {}
                Err(e) => s.error(format!("Folder picker: {e}")),
            });
        })
        .action().build(k)
        .key("change-output");
    let open = widgets::icon_button(k, "open-output", &icons::SQUARE_ARROW_UP_RIGHT, "Open Folder", |s: &mut App| s.open_output());
    let mut controls = row().gap(FLACORA.spacing.small).center_y().child(change);
    if s.settings.output != store::default_output() {
        controls = controls.child(widgets::icon_button(k, "reset-output", &icons::ROTATE_CCW, "Reset to Default Folder", |s: &mut App| {
            s.set_output(store::default_output())
        }));
    }
    controls = controls.child(open);
    let output_text = s.settings.output.to_string_lossy().into_owned();
    setting(
        k,
        "setting-folder",
        "Download Folder",
        text(output_text.clone())
            .font(k.ty.mono)
            .color(k.c.text_muted)
            .truncate()
            .help_when_truncated(output_text),
        controls,
    )
}

fn filename_row(s: &App, k: &Kit) -> El {
    let labels = Filename::ALL.map(Filename::label);
    let selected = Filename::ALL.iter().position(|value| *value == s.settings.layout.filename);
    setting(k, "setting-filename", "File Name", muted(k, "Choose how new audio files are named."),
        widgets::segmented(k, "filename", SegmentedControl::new(&labels, selected)
            .label("File Name")
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| {
                s.set_output_layout(Layout { filename: Filename::ALL[i], ..s.settings.layout });
            })))
}

fn folders_row(s: &App, k: &Kit) -> El {
    let labels = Folders::ALL.map(Folders::label);
    let selected = Folders::ALL.iter().position(|value| *value == s.settings.layout.folders);
    setting(k, "setting-folders", "Folder Organization", muted(k, "Group new downloads inside the selected folder."),
        widgets::segmented(k, "folders", SegmentedControl::new(&labels, selected)
            .label("Folder Organization")
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| {
                s.set_output_layout(Layout { folders: Folders::ALL[i], ..s.settings.layout });
            })))
}

fn preview_row(s: &App, k: &Kit) -> El {
    let example = layout_preview(&s.settings.output, s.settings.layout);
    row()
        .key("setting-preview")
        .w_full()
        .px(FLACORA.spacing.small)
        .py(FLACORA.spacing.inline)
        .gap(FLACORA.spacing.row)
        .center_y()
        .child(text("Path Preview").font(k.ty.small_medium).color(k.c.text_muted))
        .child(
            el().grow(1.0)
                .min_w(0.0)
                .px(FLACORA.spacing.row)
                .py(FLACORA.spacing.label)
                .radius(k.radius.xs)
                .fill(k.c.bg_deep)
                .border(FLACORA.border.normal, k.c.border_base)
                .child(
                    text(example.clone())
                        .key("layout-preview")
                        .font(k.ty.mono)
                        .color(k.c.text_base)
                        .truncate()
                        .help_when_truncated(example),
                ),
        )
}

fn concurrency_row(s: &App, k: &Kit) -> El {
    let labels = ["1", "2", "3", "4"];
    let current = Some(s.settings.max_concurrent_jobs.clamp(1, 4) - 1);
    setting(
        k,
        "setting-concurrency",
        "Concurrent Downloads",
        muted(k, "Run up to this many downloads at once. Browser-backed extensions still run one at a time."),
        widgets::segmented(
            k,
            "concurrent-jobs",
            SegmentedControl::new(&labels, current)
                .label("Concurrent Downloads")
                .on_pick(|s: &mut App, i: usize, _: &mut Tx| s.set_max_concurrent_jobs(i + 1)),
        ),
    )
}

fn auto_tag_row(s: &App, k: &Kit) -> El {
    let label = "Automatically Tag Downloads";
    let on = s.settings.auto_tag;
    setting(k, "setting-auto-tag", label,
        muted(k, "Update tags and cover art on new FLACs with a matching Web extension."),
        crate::astryx::switch(k, "auto-tag", label, on, App::toggle_auto_tag))
}

fn layout_preview(output: &std::path::Path, layout: Layout) -> String {
    let file = match layout.filename {
        Filename::ExtensionDefault | Filename::ArtistTitle => "HUNTR_X - Golden.flac",
        Filename::TitleArtist => "Golden - HUNTR_X.flac",
        Filename::Title => "Golden.flac",
    };
    let path = match layout.folders {
        Folders::Single => output.join(file),
        Folders::Artist => output.join("HUNTR_X").join(file),
        Folders::ArtistAlbum => output.join("HUNTR_X").join("KPop Demon Hunters").join(file),
    };
    path.to_string_lossy().into_owned()
}

fn theme_preset_row(s: &App, k: &Kit, dark: bool) -> El {
    let choices = ThemePreset::ALL.into_iter().map(|preset| {
        crate::astryx::theme_option(k, &format!("theme-preset-{}", preset.key()), preset, dark,
            s.settings.theme_preset == preset, move |s: &mut App| s.set_theme_preset(preset))
    });
    col().key("setting-theme-preset").w_full().px(FLACORA.spacing.small).py(FLACORA.settings.preset_padding_y).gap(FLACORA.spacing.label)
        .child(text("Theme").font(k.ty.ui_medium).color(k.c.text_strong))
        .child(row().key("theme-presets").role("radiogroup").label("Theme")
            .focus_group().w_full().gap(FLACORA.spacing.label).flex_wrap().children(choices))
}

fn theme_row(s: &App, k: &Kit) -> El {
    let labels = Theme::ALL.map(Theme::label);
    let current = Theme::ALL.iter().position(|t| *t == s.settings.theme);
    let detail = if s.settings.theme_preset == ThemePreset::Gothic {
        "Gothic uses a dark palette in every mode."
    } else {
        "Follow the system, or always use light or dark."
    };
    setting(
        k,
        "setting-theme",
        "Color Mode",
        muted(k, detail),
        widgets::segmented(k, "theme", SegmentedControl::new(&labels, current)
            .label("Color Mode")
            .on_pick(|s: &mut App, i: usize, _: &mut Tx| s.set_theme(Theme::ALL[i])))
            .w(FLACORA.settings.mode_width),
    )
}

/// One row per service that two or more installed extensions can handle.
fn default_rows(s: &App, k: &Kit) -> Vec<El> {
    let mut rows = Vec::new();
    for service in &SERVICES[..crate::services::PRIMARY_DOWNLOAD_COUNT] {
        let candidates = s.installed_downloaders_for(service.primary_host());
        if candidates.len() < 2 {
            continue;
        }
        let mut labels = vec!["Automatic".to_string()];
        labels.extend(candidates.iter().map(|i| i.manifest.name.clone()));
        let ids: Vec<String> = candidates.iter().map(|i| i.manifest.id.clone()).collect();
        let current = match s.settings.preferred.get(service.key) {
            Some(id) => ids.iter().position(|i| i == id).map(|p| p + 1).unwrap_or(0),
            None => 0,
        };
        let key = format!("default-{}", service.key);
        rows.push(default_setting(
            k,
            &key,
            service.name,
            format!("{} extensions can download from {}.", candidates.len(), service.name),
            default_picker(k, &key, service.name, &labels, current, ids, service.key.to_owned()),
        ));
    }
    // The resolver to use when two or more are installed.
    let mut resolvers: Vec<_> = s.installed.iter().filter(|i| i.manifest.kind == Kind::Resolver).collect();
    resolvers.sort_by_key(|i| (crate::extensions::rank(&i.manifest.id), i.manifest.name.to_lowercase()));
    if resolvers.len() > 1 {
        let mut labels = vec!["Automatic".to_string()];
        labels.extend(resolvers.iter().map(|i| i.manifest.name.clone()));
        let ids: Vec<String> = resolvers.iter().map(|i| i.manifest.id.clone()).collect();
        let current = match s.settings.preferred.get(RESOLVER_KEY) {
            Some(id) => ids.iter().position(|i| i == id).map(|p| p + 1).unwrap_or(0),
            None => 0,
        };
        rows.push(default_setting(
            k,
            "default-resolver",
            Kind::Resolver.label(),
            "Finds Spotify, Apple Music and YouTube tracks on a service you can download from.",
            default_picker(k, "default-resolver", Kind::Resolver.label(), &labels, current, ids, RESOLVER_KEY.to_owned()),
        ));
    }
    if rows.is_empty() {
        rows.push(setting(
            k,
            "setting-defaults-empty",
            "One Extension per Service",
            muted(k, "When two installed extensions can download from the same service, choose which one to use here."),
            el(),
        ));
    }
    rows
}

fn default_setting(k: &Kit, key: &str, title: &str, help: impl Into<String>, control: El) -> El {
    let help = help.into();
    let label = row().key(format!("{key}-help")).role("group").label(title.to_owned())
        .self_start().gap(FLACORA.spacing.label).center_y().description(help.clone()).help(help).help_side(rune::Side::Right)
        .child(text(title.to_owned()).font(k.ty.ui_medium).color(k.c.text_strong).a11y_hidden())
        .child(icon(&icons::CIRCLE_INFO, FLACORA.icons.inline).color(k.c.text_muted).a11y_hidden());
    setting_row(&format!("setting-{key}"), col().child(label), control).justify_end()
}

/// A segmented radio group with consistent sizing and wrapping for long extension lists.
fn default_picker(k: &Kit, key: &str, group_label: &str, labels: &[String], current: usize, ids: Vec<String>, pref_key: String) -> El {
    crate::astryx::wrapping_picker(k, key, group_label, labels, current, move |s: &mut App, i| {
        let chosen = i.checked_sub(1).and_then(|idx| ids.get(idx).cloned());
        s.set_preferred(&pref_key, chosen);
    })
}

fn ffmpeg_row(s: &App, k: &Kit) -> El {
    let (detail, status, glyph, color) = match s.tools.as_ref().map(|t| t.ffmpeg.clone()) {
        None => ("Checking…".to_string(), "Checking", &icons::CLOCK, k.c.text_muted),
        Some(Some(version)) => (version, "Ready", &icons::CIRCLE_CHECK, k.c.success.fg),
        Some(None) if store::CAN_DOWNLOAD_FFMPEG => (
            "Extensions that need FFmpeg download FFmpeg and ffprobe on first use.".into(), "On Demand", &icons::DOWNLOAD, k.c.text_muted,
        ),
        Some(None) => ("Install FFmpeg and ffprobe and add them to PATH.".into(), "Missing", &icons::TRIANGLE_ALERT, k.c.warning.fg),
    };
    setting(k, "setting-ffmpeg", "FFmpeg", muted(k, detail), helper_status(k, status, glyph, color))
}

fn browser_row(s: &App, k: &Kit) -> El {
    let (detail, status, glyph, color) = match s.tools.as_ref().map(|t| t.browser.as_ref()) {
        None => ("Checking for CloakBrowser…".into(), "Checking", &icons::CLOCK, k.c.text_muted),
        Some(Some(version)) => (version.clone(), "Installed", &icons::CIRCLE_CHECK, k.c.success.fg),
        Some(None) => (
            "CloakBrowser downloads the first time an extension needs it.".into(),
            "On Demand", &icons::DOWNLOAD, k.c.text_muted,
        ),
    };
    setting(k, "setting-browser", "Browser for Protected Sites", muted(k, detail), helper_status(k, status, glyph, color))
}

fn helper_status(k: &Kit, status: &str, glyph: &'static Icon, color: Color) -> El {
    row().gap(FLACORA.spacing.small).center_y().child(icon(glyph, FLACORA.icons.inline).color(color)).child(text(status.to_owned()).font(k.ty.small_medium).color(color))
}

fn data_row(s: &App, k: &Kit) -> El {
    let root = s.paths.root.clone();
    let root_text = root.to_string_lossy().into_owned();
    let open = widgets::icon_button(k, "open-data", &icons::SQUARE_ARROW_UP_RIGHT, "Open Folder", move |s: &mut App| {
        s.open(&root.to_string_lossy())
    });
    setting(
        k,
        "setting-data",
        "Data Folder",
        text(root_text.clone())
            .font(k.ty.mono)
            .color(k.c.text_muted)
            .truncate()
            .help_when_truncated(root_text),
        open,
    )
}

fn cover_cache_row(_s: &App, k: &Kit) -> El {
    let clear = widgets::button(
        k,
        "clear-cover-cache",
        "Clean Cache",
        Some(&icons::TRASH_2),
        ButtonVariant::Outline,
        App::clear_cover_cache,
    );
    setting(
        k,
        "setting-cover-cache",
        "Cover Art Cache",
        muted(k, "Remove unused cached album artwork from the data folder."),
        clear,
    )
}

fn history_row(s: &App, k: &Kit) -> El {
    let count = s.history.len();
    let control = if count > 0 {
        widgets::button(k, "settings-clear-history", "Clear", Some(&icons::TRASH_2), ButtonVariant::Danger, |s: &mut App| s.clear_history())
    } else {
        el()
    };
    setting(
        k,
        "setting-history",
        "Library History",
        muted(k, format!("{count} download{} remembered. Clearing it keeps the files.", if count == 1 { "" } else { "s" })),
        control,
    )
}
