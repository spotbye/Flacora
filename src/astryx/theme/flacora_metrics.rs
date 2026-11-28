//! App-owned geometry that Rune Kit's component metrics do not cover.
//! Dimensions use logical pixels; ratios and raster scales are unitless.
//! Preset-dependent colors, type and radii stay on Kit.

#[derive(Clone, Copy, Debug)]
pub struct FlacoraMetrics {
    pub window: WindowMetrics,
    pub motion: MotionMetrics,
    pub spacing: Spacing,
    pub badge: BadgeMetrics,
    pub spinner: SpinnerMetrics,
    pub artwork: ArtworkMetrics,
    pub sidebar: SidebarMetrics,
    pub page: PageMetrics,
    pub queue: QueueMetrics,
    pub library: LibraryMetrics,
    pub catalog: CatalogMetrics,
    pub settings: SettingsMetrics,
    pub overlay: OverlayMetrics,
    pub icons: IconMetrics,
    pub swatch: SwatchMetrics,
    pub border: BorderMetrics,
    pub focus: FocusMetrics,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowMetrics {
    pub min_width: f32,
    pub min_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct MotionMetrics {
    /// Response and loop duration are seconds; feather is logical pixels.
    pub extensions_response: f32,
    pub extensions_bounce: f32,
    pub extensions_feather: f32,
    pub progress_period: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub tight: f32,
    pub text: f32,
    pub small: f32,
    pub label: f32,
    pub inline: f32,
    pub row: f32,
    pub group: f32,
    pub media: f32,
    pub section: f32,
    pub inset: f32,
    pub shelf: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BadgeMetrics {
    pub height: f32,
    pub padding_x: f32,
    pub sidecar_padding_x: f32,
    pub radius: f32,
    pub status_radius: f32,
    pub interactive_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SpinnerMetrics {
    pub inline: f32,
    pub large: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ArtworkMetrics {
    pub catalog_cover: f32,
    pub job_cover: f32,
    pub library_cover: f32,
    pub catalog_row: f32,
    pub catalog_tile: f32,
    pub kind_tile: f32,
    pub kind_glyph: f32,
    pub drop_frame: f32,
    pub drop_glyph: f32,
    pub app_logo: f32,
    pub app_fallback: f32,
    pub app_glyph: f32,
    pub project_logo: f32,
    pub project_scale: f32,
    /// Placeholder glyph size relative to its artwork box.
    pub placeholder_ratio: f32,
    pub round_radius: f32,
    pub provider_inline: f32,
    pub provider_source: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SidebarMetrics {
    pub width: f32,
    pub row_height: f32,
    pub inset: f32,
    pub header_padding: f32,
    pub header_padding_top_macos: f32,
    pub row_gap: f32,
    pub brand_padding: f32,
    pub update_height: f32,
    pub update_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PageMetrics {
    pub inset: f32,
    pub content_max_width: f32,
    pub card_padding: f32,
    pub empty_text_width: f32,
    pub empty_icon: f32,
    pub header_min_width: f32,
    pub segmented_max_width: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct QueueMetrics {
    /// An estimate for virtual rows, not a fixed card height.
    pub estimated_row_height: f32,
    pub menu_min_width: f32,
    pub log_max_height: f32,
    pub destination_max_width: f32,
    pub progress_sweep_fraction: f32,
    pub route_min_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LibraryMetrics {
    pub estimated_row_height: f32,
    pub timestamp_width: f32,
    pub search_width: f32,
    pub lookup_label_width: f32,
    pub lookup_label_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CatalogMetrics {
    pub tile_width: f32,
    pub edge_fade: f32,
    pub scroll_fraction: f32,
    pub estimated_row_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SettingsMetrics {
    pub label_width: f32,
    pub preset_padding_y: f32,
    pub mode_width: f32,
    pub default_picker_max_width: f32,
    pub switch_min_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct OverlayMetrics {
    pub dialog_max_width: f32,
    pub margin: f32,
    pub toast_width: f32,
    pub toast_margin: f32,
    pub drop_inset: f32,
    pub dash_length: f32,
    pub dash_gap: f32,
    pub extension_drop_width: f32,
    pub extension_drop_height: f32,
    pub extension_dash_length: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct IconMetrics {
    pub badge: f32,
    pub small: f32,
    pub inline: f32,
    pub warning: f32,
    pub standard: f32,
    pub large: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SwatchMetrics {
    pub outer: f32,
    pub outer_radius: f32,
    pub inner: f32,
    pub inner_radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BorderMetrics {
    pub normal: f32,
    pub emphasis: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct FocusMetrics {
    pub width: f32,
    pub offset: f32,
}

/// The existing Flacora layout, shared by every Astryx preset.
pub const FLACORA: FlacoraMetrics = FlacoraMetrics {
    window: WindowMetrics {
        min_width: 1024.0,
        min_height: 600.0,
    },
    motion: MotionMetrics {
        extensions_response: 0.26,
        extensions_bounce: 0.0,
        extensions_feather: 4.0,
        progress_period: 1.4,
    },
    spacing: Spacing {
        tight: 2.0,
        text: 3.0,
        small: 4.0,
        label: 6.0,
        inline: 8.0,
        row: 10.0,
        group: 12.0,
        media: 14.0,
        section: 16.0,
        inset: 20.0,
        shelf: 22.0,
    },
    badge: BadgeMetrics {
        height: 20.0,
        padding_x: 8.0,
        sidecar_padding_x: 7.0,
        radius: 999.0,
        status_radius: 10.0,
        interactive_height: 28.0,
    },
    spinner: SpinnerMetrics {
        inline: 14.0,
        large: 24.0,
    },
    artwork: ArtworkMetrics {
        catalog_cover: 64.0,
        job_cover: 56.0,
        library_cover: 44.0,
        catalog_row: 44.0,
        catalog_tile: 134.0,
        kind_tile: 44.0,
        kind_glyph: 22.0,
        drop_frame: 56.0,
        drop_glyph: 28.0,
        app_logo: 38.0,
        app_fallback: 32.0,
        app_glyph: 18.0,
        project_logo: 18.0,
        project_scale: 3.0,
        placeholder_ratio: 0.42,
        round_radius: 999.0,
        provider_inline: 14.0,
        provider_source: 16.0,
    },
    sidebar: SidebarMetrics {
        width: 232.0,
        row_height: 36.0,
        inset: 12.0,
        header_padding: 16.0,
        header_padding_top_macos: 40.0,
        row_gap: 4.0,
        brand_padding: 6.0,
        update_height: 28.0,
        update_radius: 10.0,
    },
    page: PageMetrics {
        inset: 24.0,
        content_max_width: 1120.0,
        card_padding: 12.0,
        empty_text_width: 360.0,
        empty_icon: 32.0,
        header_min_width: 220.0,
        segmented_max_width: 760.0,
    },
    queue: QueueMetrics {
        estimated_row_height: 96.0,
        menu_min_width: 180.0,
        log_max_height: 180.0,
        destination_max_width: 300.0,
        progress_sweep_fraction: 0.30,
        route_min_height: 30.0,
    },
    library: LibraryMetrics {
        estimated_row_height: 65.0,
        timestamp_width: 76.0,
        search_width: 240.0,
        lookup_label_width: 112.0,
        lookup_label_height: 32.0,
    },
    catalog: CatalogMetrics {
        tile_width: 152.0,
        edge_fade: 12.0,
        scroll_fraction: 0.85,
        estimated_row_height: 61.0,
    },
    settings: SettingsMetrics {
        label_width: 168.0,
        preset_padding_y: 7.0,
        mode_width: 248.0,
        default_picker_max_width: 520.0,
        switch_min_height: 28.0,
    },
    overlay: OverlayMetrics {
        dialog_max_width: 416.0,
        margin: 16.0,
        toast_width: 340.0,
        toast_margin: 24.0,
        drop_inset: 28.0,
        dash_length: 8.0,
        dash_gap: 4.0,
        extension_drop_width: 560.0,
        extension_drop_height: 224.0,
        extension_dash_length: 6.0,
    },
    icons: IconMetrics {
        badge: 12.0,
        small: 13.0,
        inline: 14.0,
        warning: 15.0,
        standard: 16.0,
        large: 18.0,
    },
    swatch: SwatchMetrics {
        outer: 18.0,
        outer_radius: 4.0,
        inner: 8.0,
        inner_radius: 2.0,
    },
    border: BorderMetrics {
        normal: 1.0,
        emphasis: 2.0,
    },
    focus: FocusMetrics {
        width: 2.0,
        offset: 2.0,
    },
};
