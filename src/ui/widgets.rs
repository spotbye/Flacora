//! Flacora-specific labels, extension choices and provider assets.
use super::theme::Kit;
use crate::app::App;
use crate::extensions::ExtOption;
use crate::services::Service;
use crate::astryx::SegmentedControl;
use rune::{El, SurfaceFit, Tx, el};
pub use crate::astryx::{page, page_header, page_header_with_subtitle, section, card, badge, badge_with_icon, segmented, filter_group, filter_button, error_banner, scroll_bar_style, page_scroll, status_badge, cover, icon_button, clear_input_button, danger_icon_button, button, empty_state};

/// Formats extension-provided captions for display while preserving mixed-case names and paths.
pub fn title_case_label(label: &str) -> String {
    let mut words = label.split_whitespace().peekable();
    let mut title = String::with_capacity(label.len());
    while let Some(word) = words.next() {
        let middle = !title.is_empty() && words.peek().is_some();
        if !title.is_empty() {
            title.push(' ');
        }
        let connector = matches!(word, "a" | "an" | "and" | "as" | "at" | "by" | "for" | "from" | "in" | "of" | "on" | "or" | "per" | "the" | "to" | "via" | "with");
        if (middle && connector) || word.contains(['.', '/', '\\', '@']) || word.chars().any(char::is_uppercase) {
            title.push_str(word);
        } else if let Some(first) = word.chars().next() {
            title.extend(first.to_uppercase());
            title.push_str(&word[first.len_utf8()..]);
        }
    }
    title
}

pub fn extension_option(k: &Kit, s: &App, extension: &str, option: &ExtOption, key: &str) -> El {
    let labels: Vec<String> = option.choices.iter().map(|choice| title_case_label(&choice.label)).collect();
    let selected = s.pick_index(extension, option);
    let (extension, option_key) = (extension.to_owned(), option.key.clone());
    segmented(k, key, SegmentedControl::new(&labels, Some(selected))
        .label(title_case_label(&option.label))
        .on_pick(move |s: &mut App, i: usize, _: &mut Tx| s.set_pick(&extension, &option_key, i)))
}

/// The detected service's name, or the supplied label.
pub fn service_chip(k: &Kit, service: Option<&Service>, fallback: &str) -> El {
    badge(k, service.map_or(fallback, |s| s.name))
}

pub fn provider_logo(s: &App, service: &Service, size: f32) -> Option<El> {
    let image = s.provider_logos.get(service.key).filter(|image| image.error().is_none())?;
    Some(el().key(format!("provider-icon-{}", service.key)).size(size, size).pixels_fit(image.pixels(), SurfaceFit::Fit))
}

/// "Just Now", "5 min ago", "3 h ago", "2 d ago", else a date.
pub fn ago(date: u64, now: u64) -> String {
    let seconds = now.saturating_sub(date);
    match seconds {
        0..60 => "Just Now".into(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        86_400..2_592_000 => format!("{} d ago", seconds / 86_400),
        _ => {
            let days = date / 86_400;
            // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
            let z = days as i64 + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            format!("{year}-{month:02}-{day:02}")
        }
    }
}

/// Flacora owns cancellation; Astryx owns the dialog and focus behavior.
pub fn confirmation_dialog(k: &Kit, key: &str, title: &str, detail: String, trigger: &str, keep: El, confirm: El) -> El {
    crate::astryx::confirmation_dialog(k, key, title, detail, trigger, [keep, confirm], |s: &mut App| s.confirm_action = None)
}
