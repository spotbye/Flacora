//! Lucide 0.577.0 SVG geometry (https://github.com/lucide-icons/lucide/tree/0.577.0/icons).
//! See assets/lucide.LICENSE for the upstream license.

use rune::Icon;

macro_rules! lucide {
    ($constant:ident, $name:literal, $body:literal) => {
        pub static $constant: Icon = Icon {
            name: concat!("lucide-", $name),
            view: 24.0,
            body: concat!(
                r#"<g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"#,
                $body,
                "</g>",
            ),
        };
    };
}

lucide!(ACTIVITY, "activity", r#"<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>"#);
lucide!(ARROW_RIGHT, "arrow-right", r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#);
lucide!(ARROW_UP, "arrow-up", r#"<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>"#);
lucide!(CHECK, "check", r#"<path d="M20 6 9 17l-5-5"/>"#);
lucide!(CHEVRON_DOWN, "chevron-down", r#"<path d="m6 9 6 6 6-6"/>"#);
lucide!(CHEVRON_LEFT, "chevron-left", r#"<path d="m15 18-6-6 6-6"/>"#);
lucide!(CHEVRON_RIGHT, "chevron-right", r#"<path d="m9 18 6-6-6-6"/>"#);
lucide!(CHEVRONS_UP_DOWN, "chevrons-up-down", r#"<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>"#);
lucide!(CIRCLE_CHECK, "circle-check", r#"<circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/>"#);
lucide!(CIRCLE_INFO, "circle-info", r#"<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>"#);
lucide!(CIRCLE_X, "circle-x", r#"<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>"#);
lucide!(CLOCK, "clock", r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#);
lucide!(CLOUD_DOWNLOAD, "cloud-download", r#"<path d="M12 13v8l-4-4"/><path d="m12 21 4-4"/><path d="M4.393 15.269A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.436 8.284"/>"#);
lucide!(COFFEE, "coffee", r#"<path d="M10 2v2"/><path d="M14 2v2"/><path d="M16 8a1 1 0 0 1 1 1v8a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4V9a1 1 0 0 1 1-1h14a4 4 0 1 1 0 8h-1"/><path d="M6 2v2"/>"#);
lucide!(COPY, "copy", r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#);
lucide!(DISC, "disc", r#"<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>"#);
lucide!(DOWNLOAD, "download", r#"<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>"#);
lucide!(ELLIPSIS, "ellipsis", r#"<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>"#);
lucide!(FILE_UP, "file-up", r#"<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M12 12v6"/><path d="m15 15-3-3-3 3"/>"#);
lucide!(FOLDER, "folder", r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>"#);
lucide!(LAYOUT_GRID, "layout-grid", r#"<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>"#);
lucide!(LIBRARY, "library", r#"<path d="m16 6 4 14"/><path d="M12 6v14"/><path d="M8 8v12"/><path d="M4 4v16"/>"#);
lucide!(LINK, "link", r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>"#);
lucide!(LIST_CHECKS, "list-checks", r#"<path d="m3 17 2 2 4-4"/><path d="m3 7 2 2 4-4"/><path d="M13 6h8"/><path d="M13 12h8"/><path d="M13 18h8"/>"#);
lucide!(LIST_MUSIC, "list-music", r#"<path d="M16 5H3"/><path d="M11 12H3"/><path d="M11 19H3"/><path d="M21 16V5"/><circle cx="18" cy="16" r="3"/>"#);
lucide!(MIC_VOCAL, "mic-vocal", r#"<path d="m11 7.601-5.994 8.19a1 1 0 0 0 .1 1.298l.817.818a1 1 0 0 0 1.314.087L15.09 12"/><path d="M16.5 21.174C15.5 20.5 14.372 20 13 20c-2.058 0-3.928 2.356-6 2-2.072-.356-2.775-3.369-1.5-4.5"/><circle cx="16" cy="7" r="5"/>"#);
lucide!(MUSIC, "music", r#"<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>"#);
lucide!(PAUSE, "pause", r#"<rect x="14" y="3" width="5" height="18" rx="1"/><rect x="5" y="3" width="5" height="18" rx="1"/>"#);
lucide!(PLAY, "play", r#"<path d="M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"/>"#);
lucide!(PLUS, "plus", r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#);
lucide!(PUZZLE, "puzzle", r#"<path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"/>"#);
lucide!(REFRESH_CW, "refresh-cw", r#"<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>"#);
lucide!(ROTATE_CCW, "rotate-ccw", r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>"#);
lucide!(SEARCH, "search", r#"<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>"#);
lucide!(SETTINGS, "settings", r#"<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>"#);
lucide!(SHARE_2, "share-2", r#"<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" x2="15.42" y1="13.51" y2="17.49"/><line x1="15.41" x2="8.59" y1="6.51" y2="10.49"/>"#);
lucide!(SQUARE, "square", r#"<rect width="18" height="18" x="3" y="3" rx="2"/>"#);
lucide!(SQUARE_ARROW_UP_RIGHT, "square-arrow-up-right", r#"<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M8 8h8v8"/><path d="m8 16 8-8"/>"#);
lucide!(TAG, "tag", r#"<path d="M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z"/><circle cx="7.5" cy="7.5" r=".5" fill="currentColor"/>"#);
lucide!(TRASH_2, "trash-2", r#"<path d="M10 11v6"/><path d="M14 11v6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>"#);
lucide!(TRIANGLE_ALERT, "triangle-alert", r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#);
lucide!(X, "x", r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#);
