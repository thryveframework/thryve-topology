//! Colours, fonts and panel styles, taken from Thryve's website: a deep navy with teal and blue
//! accents in the dark theme, and their light counterparts.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Shadow,
    Stroke, TextStyle, Visuals,
};

/// Dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The website's default: navy, teal and blue.
    Dark,
    /// Pale blue-grey, with deeper teal and blue.
    Light,
}

/// The colours of one theme. The names follow the website's style sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Which theme these colours belong to.
    pub mode: Mode,
    /// The page behind everything.
    pub bg: Color32,
    /// A second, slightly lighter background, for the corners' glow.
    pub bg2: Color32,
    /// Frosted glass: the panels.
    pub surface: Color32,
    /// The cards: frosted glass, a little more solid than the panels so their text stays sharp.
    pub card: Color32,
    /// Main text.
    pub fg: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// Thin borders.
    pub border: Color32,
    /// Borders that stand out: a hovered card, the default-file note.
    pub border_strong: Color32,
    /// Teal: starting points, selection, the brand.
    pub accent: Color32,
    /// Blue: tasks and edges.
    pub accent2: Color32,
    /// Teal, for the busiest edges (the website's vein colour).
    pub vein: Color32,
    /// Blue, for the quietest edges (the website's second vein colour).
    pub vein2: Color32,
    /// Where graph mode stopped.
    pub danger: Color32,
}

impl Palette {
    /// The dark theme, as on the website.
    pub fn dark() -> Self {
        Self {
            mode: Mode::Dark,
            bg: Color32::from_rgb(0x04, 0x07, 0x0f),
            bg2: Color32::from_rgb(0x06, 0x0c, 0x1a),
            surface: Color32::from_rgba_unmultiplied(10, 18, 34, 150),
            card: Color32::from_rgba_unmultiplied(9, 16, 31, 225),
            fg: Color32::from_rgb(0xdc, 0xe7, 0xf6),
            muted: Color32::from_rgb(0x82, 0x96, 0xb2),
            border: Color32::from_rgba_unmultiplied(120, 165, 225, 40),
            border_strong: Color32::from_rgba_unmultiplied(130, 190, 235, 90),
            accent: Color32::from_rgb(0x22, 0xe0, 0xd0),
            accent2: Color32::from_rgb(0x3a, 0x9b, 0xff),
            vein: Color32::from_rgb(0x24, 0xe6, 0xd4),
            vein2: Color32::from_rgb(0x1f, 0x78, 0xe6),
            danger: Color32::from_rgb(0xff, 0x6b, 0x7d),
        }
    }

    /// The light theme, as on the website.
    pub fn light() -> Self {
        Self {
            mode: Mode::Light,
            bg: Color32::from_rgb(0xee, 0xf3, 0xfb),
            bg2: Color32::from_rgb(0xe4, 0xec, 0xf8),
            surface: Color32::from_rgba_unmultiplied(255, 255, 255, 185),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 236),
            fg: Color32::from_rgb(0x0b, 0x15, 0x24),
            muted: Color32::from_rgb(0x4a, 0x5b, 0x73),
            border: Color32::from_rgba_unmultiplied(20, 55, 95, 45),
            border_strong: Color32::from_rgba_unmultiplied(15, 90, 130, 95),
            accent: Color32::from_rgb(0x0d, 0x9c, 0x8d),
            accent2: Color32::from_rgb(0x16, 0x67, 0xd6),
            vein: Color32::from_rgb(0x0f, 0xae, 0x9e),
            vein2: Color32::from_rgb(0x2f, 0x74, 0xd8),
            danger: Color32::from_rgb(0xd6, 0x45, 0x50),
        }
    }

    /// The palette for `mode`.
    pub fn of(mode: Mode) -> Self {
        match mode {
            Mode::Dark => Self::dark(),
            Mode::Light => Self::light(),
        }
    }

    /// A frosted panel, like the website's `.glass`.
    pub fn glass(&self) -> Frame {
        Frame::new()
            .fill(self.surface)
            .stroke(Stroke::new(1.0_f32, self.border))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(14))
            .shadow(Shadow {
                offset: [0, 10],
                blur: 30,
                spread: 0,
                color: Color32::from_black_alpha(match self.mode {
                    Mode::Dark => 90,
                    Mode::Light => 18,
                }),
            })
    }
}

/// The font family for headings and names: semibold where the system has it.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Sets egui's colours, spacing and text sizes for `palette`.
pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut visuals = match palette.mode {
        Mode::Dark => Visuals::dark(),
        Mode::Light => Visuals::light(),
    };
    visuals.override_text_color = Some(palette.fg);
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = palette.card;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.extreme_bg_color = palette.bg2.gamma_multiply(0.9);
    visuals.faint_bg_color = palette.surface;
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.warn_fg_color = palette.danger;
    visuals.error_fg_color = palette.danger;

    let radius = CornerRadius::same(9);
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.fg);
    widgets.inactive.weak_bg_fill = palette.border.gamma_multiply(0.6);
    widgets.inactive.bg_fill = palette.border.gamma_multiply(0.6);
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.fg);
    widgets.hovered.weak_bg_fill = palette.accent.gamma_multiply(0.16);
    widgets.hovered.bg_fill = palette.accent.gamma_multiply(0.16);
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.accent.gamma_multiply(0.7));
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.fg);
    widgets.active.weak_bg_fill = palette.accent.gamma_multiply(0.28);
    widgets.active.bg_fill = palette.accent.gamma_multiply(0.28);
    widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.fg);
    widgets.open.weak_bg_fill = palette.accent.gamma_multiply(0.2);
    for state in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = radius;
    }

    ctx.set_theme(match palette.mode {
        Mode::Dark => egui::Theme::Dark,
        Mode::Light => egui::Theme::Light,
    });
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.text_styles = [
            (TextStyle::Heading, FontId::new(19.0, semibold())),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

/// Uses the system's own fonts where they're found (Segoe UI and Cascadia Mono or Consolas on
/// Windows), keeping egui's fonts after them for any missing character. Elsewhere, egui's fonts
/// are used as they are.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fonts_dir = std::path::Path::new(r"C:\Windows\Fonts");
    let mut load = |name: &str, files: &[&str]| -> bool {
        for file in files {
            if let Ok(bytes) = std::fs::read(fonts_dir.join(file)) {
                fonts
                    .font_data
                    .insert(name.to_string(), Arc::new(FontData::from_owned(bytes)));
                return true;
            }
        }
        false
    };

    let has_sans = load("system-sans", &["segoeui.ttf"]);
    let has_semibold = load("system-semibold", &["seguisb.ttf", "segoeuib.ttf"]);
    let has_mono = load("system-mono", &["CascadiaMono.ttf", "consola.ttf"]);

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    if has_sans {
        proportional.insert(0, "system-sans".to_string());
    }
    let proportional = proportional.clone();

    if has_mono {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "system-mono".to_string());
    }

    // The semibold family always exists, so text that asks for it never fails; without a
    // semibold font it's the regular one.
    let mut semibold_list = proportional;
    if has_semibold {
        semibold_list.insert(0, "system-semibold".to_string());
    }
    fonts.families.insert(semibold(), semibold_list);

    ctx.set_fonts(fonts);
}
