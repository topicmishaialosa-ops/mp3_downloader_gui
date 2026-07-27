use eframe::egui::{self, Color32, Frame, Margin, Rounding, Stroke, Vec2};

#[derive(Clone, Copy)]
pub struct AppTheme {
    pub window_bg: Color32,
    pub card_bg: Color32,
    pub card_border: Color32,
    pub header_bg: Color32,
    pub header_btn_bg: Color32,
    pub status_bg: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_on_header: Color32,
    pub accent: Color32,
    pub btn_primary: Color32,
    pub btn_success: Color32,
    pub btn_neutral: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub link: Color32,
    pub stripe: Color32,
    pub progress: Color32,
    pub separator: Color32,
}

impl AppTheme {
    pub fn dark() -> Self {
        Self {
            window_bg: Color32::from_rgb(22, 24, 32),
            card_bg: Color32::from_rgb(34, 37, 50),
            card_border: Color32::from_rgb(58, 62, 82),
            header_bg: Color32::from_rgb(36, 52, 88),
            header_btn_bg: Color32::from_rgb(52, 72, 118),
            status_bg: Color32::from_rgb(28, 30, 42),
            text_primary: Color32::from_rgb(232, 235, 248),
            text_secondary: Color32::from_rgb(175, 182, 210),
            text_muted: Color32::from_rgb(120, 128, 155),
            text_on_header: Color32::WHITE,
            accent: Color32::from_rgb(110, 165, 255),
            btn_primary: Color32::from_rgb(58, 118, 210),
            btn_success: Color32::from_rgb(42, 145, 95),
            btn_neutral: Color32::from_rgb(55, 60, 78),
            success: Color32::from_rgb(80, 210, 130),
            warning: Color32::from_rgb(240, 190, 70),
            error: Color32::from_rgb(240, 95, 95),
            link: Color32::from_rgb(130, 185, 255),
            stripe: Color32::from_rgb(40, 43, 58),
            progress: Color32::from_rgb(70, 140, 230),
            separator: Color32::from_rgb(50, 54, 72),
        }
    }

    pub fn light() -> Self {
        Self {
            window_bg: Color32::from_rgb(242, 244, 249),
            card_bg: Color32::WHITE,
            card_border: Color32::from_rgb(210, 216, 228),
            header_bg: Color32::from_rgb(32, 68, 128),
            header_btn_bg: Color32::from_rgb(48, 88, 155),
            status_bg: Color32::from_rgb(232, 236, 244),
            text_primary: Color32::from_rgb(28, 32, 48),
            text_secondary: Color32::from_rgb(70, 78, 100),
            text_muted: Color32::from_rgb(130, 138, 158),
            text_on_header: Color32::WHITE,
            accent: Color32::from_rgb(42, 88, 168),
            btn_primary: Color32::from_rgb(48, 108, 195),
            btn_success: Color32::from_rgb(38, 140, 88),
            btn_neutral: Color32::from_rgb(220, 224, 234),
            success: Color32::from_rgb(28, 150, 75),
            warning: Color32::from_rgb(190, 130, 20),
            error: Color32::from_rgb(200, 55, 55),
            link: Color32::from_rgb(35, 95, 185),
            stripe: Color32::from_rgb(248, 249, 252),
            progress: Color32::from_rgb(50, 120, 210),
            separator: Color32::from_rgb(220, 224, 232),
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.window_bg.r() < 128 {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.window_fill = self.window_bg;
        visuals.panel_fill = self.window_bg;
        visuals.extreme_bg_color = self.card_bg;
        visuals.faint_bg_color = self.stripe;
        visuals.widgets.noninteractive.bg_fill = self.card_bg;
        visuals.widgets.inactive.bg_fill = self.card_bg;
        visuals.widgets.hovered.bg_fill = self.header_btn_bg;
        visuals.widgets.active.bg_fill = self.btn_primary;
        visuals.widgets.open.bg_fill = self.btn_primary;
        visuals.widgets.noninteractive.fg_stroke.color = self.text_primary;
        visuals.widgets.inactive.fg_stroke.color = self.text_primary;
        visuals.widgets.hovered.fg_stroke.color = self.text_on_header;
        visuals.widgets.active.fg_stroke.color = self.text_on_header;
        visuals.override_text_color = Some(self.text_primary);
        visuals.hyperlink_color = self.link;
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.35);
        visuals.selection.stroke.color = self.accent;
        visuals.widgets.noninteractive.weak_bg_fill = self.status_bg;
        visuals.widgets.inactive.weak_bg_fill = self.status_bg;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.separator);
        visuals.window_stroke.color = self.card_border;
        visuals.window_shadow = egui::epaint::Shadow::NONE;

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(14.0, 7.0);
        style.spacing.indent = 18.0;
        style.visuals.widgets.inactive.rounding = Rounding::same(6.0);
        style.visuals.widgets.active.rounding = Rounding::same(6.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(6.0);
        ctx.set_style(style);
    }

    pub fn card(&self) -> Frame {
        Frame {
            fill: self.card_bg,
            rounding: Rounding::same(10.0),
            stroke: Stroke::new(1.0, self.card_border),
            inner_margin: Margin::symmetric(14.0, 12.0),
            ..Default::default()
        }
    }

    pub fn status_bar(&self) -> Frame {
        Frame {
            fill: self.status_bg,
            rounding: Rounding::same(8.0),
            stroke: Stroke::new(1.0, self.card_border),
            inner_margin: Margin::symmetric(12.0, 8.0),
            ..Default::default()
        }
    }

    pub fn header_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(self.text_on_header))
            .fill(self.header_btn_bg)
            .rounding(Rounding::same(6.0))
    }

    pub fn primary_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
            .fill(self.btn_primary)
            .rounding(Rounding::same(6.0))
    }

    pub fn success_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        egui::Button::new(egui::RichText::new(label).color(Color32::WHITE))
            .fill(self.btn_success)
            .rounding(Rounding::same(6.0))
    }

    pub fn neutral_button<'a>(&self, label: &'a str) -> egui::Button<'a> {
        let text = if self.window_bg.r() < 128 {
            self.text_primary
        } else {
            self.text_secondary
        };
        egui::Button::new(egui::RichText::new(label).color(text))
            .fill(self.btn_neutral)
            .rounding(Rounding::same(6.0))
    }

    pub fn section_title(&self, text: &str) -> egui::RichText {
        egui::RichText::new(text)
            .size(15.0)
            .strong()
            .color(self.text_primary)
    }

    pub fn status_color(&self, status: &str, loading: bool) -> Color32 {
        if loading {
            self.warning
        } else if status.starts_with('✅') || status.starts_with("📋") {
            self.success
        } else if status.starts_with('❌') || status.starts_with('⚠') {
            self.error
        } else {
            self.text_secondary
        }
    }
}
