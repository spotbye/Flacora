//! Astryx adapters own Rune Kit construction and component profiles.
use super::{Kit, theme::FLACORA};
use rune::{Color, El, Icon, TextEdit, TextStyle, Tx};
use rune_kit::{ButtonVariant, ControlSize, IconButtonVariant, Placement};

pub struct Button {
    inner: rune_kit::Button,
    profile: ButtonProfile,
    variant: ButtonVariant,
    disabled: bool,
}
impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            inner: rune_kit::Button::new(label),
            profile: ButtonProfile::Base,
            variant: ButtonVariant::Neutral,
            disabled: false,
        }
    }
    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.inner = self.inner.variant(v);
        self.variant = v;
        self
    }
    pub fn control_size(mut self, s: ControlSize) -> Self {
        self.inner = self.inner.control_size(s);
        self
    }
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.inner = self.inner.icon(i);
        self
    }
    pub fn label_with(mut self, f: impl FnOnce(TextStyle, Color) -> El + 'static) -> Self {
        self.inner = self.inner.label_with(f);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.inner = self.inner.disabled(d);
        self.disabled = d;
        self
    }
    pub fn expanded(mut self, on: bool) -> Self {
        self.inner = self.inner.expanded(on);
        self
    }
    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.inner = self.inner.help(text);
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_press(f);
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_press_tx(f);
        self
    }
    pub fn action(mut self) -> Self {
        self.profile = ButtonProfile::Action;
        self
    }
    pub fn compact(mut self) -> Self {
        self.profile = ButtonProfile::Compact;
        self
    }
    pub fn toolbar(mut self) -> Self {
        self.profile = ButtonProfile::Toolbar;
        self
    }
    pub fn build(self, k: &Kit) -> El {
        let danger = self.variant == ButtonVariant::Danger;
        let profile = match self.profile {
            ButtonProfile::Base => &**k,
            ButtonProfile::Action if danger => &k.danger_button,
            ButtonProfile::Action => &k.controls,
            ButtonProfile::Compact => &k.compact,
            ButtonProfile::Toolbar => &k.toolbar,
        };
        let el = self.inner.build(profile);
        if danger && !self.disabled { el.fill(k.c.danger.bg) } else { el }
    }
}

pub struct IconButton {
    inner: rune_kit::IconButton,
    danger: bool,
    disabled: bool,
    expanded: Option<bool>,
}
impl IconButton {
    pub fn new(i: &'static Icon) -> Self {
        Self {
            inner: rune_kit::IconButton::new(i),
            danger: false,
            disabled: false,
            expanded: None,
        }
    }
    pub fn variant(mut self, v: IconButtonVariant) -> Self {
        self.inner = self.inner.variant(v);
        self
    }
    pub fn control_size(mut self, s: ControlSize) -> Self {
        self.inner = self.inner.control_size(s);
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.inner = self.inner.label(l);
        self
    }
    pub fn help(mut self, text: impl Into<String>) -> Self {
        self.inner = self.inner.help(text);
        self
    }
    #[allow(dead_code)]
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    #[allow(dead_code)]
    pub fn expanded(mut self, on: bool) -> Self {
        self.expanded = Some(on);
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_press(f);
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_press_tx(f);
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    pub fn build(self, k: &Kit) -> El {
        let mut el = self.inner
            .build(if self.danger { &k.danger_icon } else { k });
        if self.disabled {
            el = el.disabled(true);
        }
        if let Some(on) = self.expanded {
            el = el.expanded(on);
        }
        el
    }
}

pub struct TextInput {
    inner: rune_kit::TextInput,
}
impl TextInput {
    pub fn new(edit: &TextEdit) -> Self {
        Self {
            inner: rune_kit::TextInput::new(edit),
        }
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.inner = self.inner.placeholder(p);
        self
    }
    pub fn large(mut self) -> Self {
        self.inner = self.inner.large();
        self
    }
    pub fn width(mut self, w: Option<f32>) -> Self {
        self.inner = self.inner.width(w);
        self
    }
    pub fn leading_icon(mut self, i: &'static Icon) -> Self {
        self.inner = self.inner.leading_icon(i);
        self
    }
    pub fn trailing(mut self, e: El) -> Self {
        self.inner = self.inner.trailing(e);
        self
    }
    pub fn on_edit<S: 'static>(
        mut self,
        f: impl for<'a> Fn(&'a mut S) -> &'a mut TextEdit + 'static,
    ) -> Self {
        self.inner = self.inner.on_edit(f);
        self
    }
    pub fn on_submit<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_submit(f);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner.build(k)
    }
}

pub struct SegmentedControl {
    inner: rune_kit::SegmentedControl,
}
impl SegmentedControl {
    pub fn new<S: AsRef<str>>(labels: &[S], selected: Option<usize>) -> Self {
        Self {
            inner: rune_kit::SegmentedControl::new(labels, selected),
        }
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.inner = self.inner.label(l);
        self
    }
    pub fn on_pick<S: 'static>(
        mut self,
        f: impl Fn(&mut S, usize, &mut Tx) + Clone + 'static,
    ) -> Self {
        self.inner = self.inner.on_pick(f);
        self
    }
    pub fn build(self, k: &Kit, key: &str) -> El {
        rune::row()
            .key(format!("{key}-track"))
            .padding(FLACORA.spacing.small)
            .radius(k.radius.md)
            .fill(k.c.layer_01)
            .child(
                self.inner
                    .fit()
                    .build(&k.segmented)
                    .grow(1.0)
                    .max_w(FLACORA.page.segmented_max_width)
                    .key(key.to_owned()),
            )
    }
}

pub struct Divider {
    inner: rune_kit::Divider,
}
impl Divider {
    pub fn horizontal() -> Self {
        Self {
            inner: rune_kit::Divider::horizontal(),
        }
    }
    pub fn vertical() -> Self {
        Self {
            inner: rune_kit::Divider::vertical(),
        }
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner.build(k)
    }
}

pub struct Spinner {
    inner: rune_kit::Spinner,
}
impl Spinner {
    pub fn new() -> Self {
        Self {
            inner: rune_kit::Spinner::new(),
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.inner = self.inner.size(s);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner.build(k)
    }
}

pub struct Collapsible {
    inner: rune_kit::Collapsible,
}
impl Collapsible {
    pub fn new(title: impl Into<String>, open: bool) -> Self {
        Self {
            inner: rune_kit::Collapsible::new(title, open),
        }
    }
    pub fn child(mut self, c: El) -> Self {
        self.inner = self.inner.child(c);
        self
    }
    pub fn on_toggle_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_toggle_tx(f);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner.build(k)
    }
}

pub struct Sidebar {
    inner: rune_kit::Sidebar,
}
impl Sidebar {
    pub fn new() -> Self {
        Self {
            inner: rune_kit::Sidebar::new(),
        }
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.inner = self.inner.label(l);
        self
    }
    pub fn header(mut self, e: El) -> Self {
        self.inner = self.inner.header(e);
        self
    }
    pub fn footer(mut self, e: El) -> Self {
        self.inner = self.inner.footer(e);
        self
    }
    pub fn child(mut self, e: El) -> Self {
        self.inner = self.inner.child(e);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        let header_top = if cfg!(target_os = "macos") {
            FLACORA.sidebar.header_padding_top_macos
        } else {
            FLACORA.sidebar.header_padding
        };
        self.inner
            .inset(FLACORA.sidebar.inset)
            .header_padding(header_top, FLACORA.sidebar.header_padding)
            .row_gap(FLACORA.sidebar.row_gap)
            .build(k)
            .w(FLACORA.sidebar.width)
            .fill(k.c.bg_base)
    }
}

pub struct SidebarRow {
    inner: rune_kit::SidebarRow,
    selected: bool,
}
impl SidebarRow {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            inner: rune_kit::SidebarRow::new(label),
            selected: false,
        }
    }
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.inner = self.inner.icon(i);
        self
    }
    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.inner = self.inner.badge(b);
        self
    }
    pub fn selected(mut self, s: bool) -> Self {
        self.inner = self.inner.selected(s);
        self.selected = s;
        self
    }
    pub fn on_select<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_select(f);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        let el = self
            .inner
            .height(FLACORA.sidebar.row_height)
            .title_style(if self.selected {
                k.ty.body_medium
            } else {
                k.ty.ui
            })
            .build(k);
        if self.selected {
            el.fill(k.c.layer_04)
        } else {
            el
        }
    }
}

pub struct Menu {
    inner: rune_kit::Menu,
}
impl Menu {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            inner: rune_kit::Menu::new(key),
        }
    }
    pub fn trigger(mut self, t: El) -> Self {
        self.inner = self.inner.trigger(t);
        self
    }
    pub fn open(mut self, o: bool) -> Self {
        self.inner = self.inner.open(o);
        self
    }
    pub fn placement(mut self, p: Placement) -> Self {
        self.inner = self.inner.placement(p);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.inner = self.inner.min_width(w);
        self
    }
    pub fn on_close<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_close(f);
        self
    }
    pub fn item(mut self, item: MenuItem) -> Self {
        self.inner = self.inner.item(item.inner);
        self
    }
    pub fn build(self, k: &Kit, overlays: &mut Vec<El>) -> El {
        self.inner.build(k, overlays)
    }
}

pub struct MenuItem {
    inner: rune_kit::MenuItem,
}
impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            inner: rune_kit::MenuItem::new(label),
        }
    }
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.inner = self.inner.icon(i);
        self
    }
    pub fn danger(mut self, on: bool) -> Self {
        self.inner = self.inner.danger(on);
        self
    }
    pub fn on_select<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.inner = self.inner.on_select(f);
        self
    }
    pub fn on_select_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.inner = self.inner.on_select_tx(f);
        self
    }
}

pub struct ToastRegion {
    inner: rune_kit::ToastRegion,
}
impl ToastRegion {
    pub fn new() -> Self {
        Self {
            inner: rune_kit::ToastRegion::new(),
        }
    }
    pub fn toast(mut self, t: El) -> Self {
        self.inner = self.inner.toast(t);
        self
    }
    pub fn build(self, k: &Kit) -> El {
        self.inner.build(k)
    }
}

enum ButtonProfile {
    Base,
    Action,
    Compact,
    Toolbar,
}
