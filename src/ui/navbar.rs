use egui::{Key, Response, RichText, Ui};

pub struct NavbarAction {
    pub navigate_to: Option<String>,
    pub go_back: bool,
    pub go_forward: bool,
    pub reload: bool,
    pub go_home: bool,
    pub zoom_in: bool,
    pub zoom_out: bool,
    pub zoom_reset: bool,
    pub new_tab: bool,
    pub close_tab: Option<usize>,
    pub switch_tab: Option<usize>,
}

impl Default for NavbarAction {
    fn default() -> Self {
        Self {
            navigate_to: None,
            go_back: false,
            go_forward: false,
            reload: false,
            go_home: false,
            zoom_in: false,
            zoom_out: false,
            zoom_reset: false,
            new_tab: false,
            close_tab: None,
            switch_tab: None,
        }
    }
}

pub fn render_tabs(
    ui: &mut Ui,
    tab_titles: &[String],
    active_tab: usize,
    action: &mut NavbarAction,
) {
    ui.horizontal(|ui| {
        for (idx, title) in tab_titles.iter().enumerate() {
            let is_active = idx == active_tab;
            let display_title = if title.is_empty() { "Untitled" } else { title };
            let short_title: String = display_title.chars().take(22).collect();

            let bg_color = if is_active {
                ui.visuals().window_fill()
            } else {
                ui.visuals().faint_bg_color
            };

            egui::Frame::none()
                .fill(bg_color)
                .rounding(egui::Rounding {
                    nw: 6.0,
                    ne: 6.0,
                    sw: 0.0,
                    se: 0.0,
                })
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let btn = ui.selectable_label(is_active, RichText::new(&short_title).strong());
                        if btn.clicked() {
                            action.switch_tab = Some(idx);
                        }

                        if tab_titles.len() > 1 {
                            let close_btn = ui.small_button("✕");
                            if close_btn.clicked() {
                                action.close_tab = Some(idx);
                            }
                        }
                    });
                });
            ui.add_space(2.0);
        }

        if ui.button("+").on_hover_text("เปิดแท็บใหม่").clicked() {
            action.new_tab = true;
        }
    });
}

pub fn render_navbar(
    ui: &mut Ui,
    url_input: &mut String,
    can_back: bool,
    can_forward: bool,
    zoom: f32,
    is_loading: bool,
    action: &mut NavbarAction,
) -> Response {
    ui.horizontal(|ui| {
        // Navigation Buttons
        if ui.add_enabled(can_back, egui::Button::new("◀")).on_hover_text("ย้อนกลับ").clicked() {
            action.go_back = true;
        }
        if ui.add_enabled(can_forward, egui::Button::new("▶")).on_hover_text("ไปข้างหน้า").clicked() {
            action.go_forward = true;
        }
        let reload_icon = if is_loading { "⏳" } else { "🔄" };
        if ui.button(reload_icon).on_hover_text("โหลดหน้าใหม่").clicked() {
            action.reload = true;
        }
        if ui.button("🏠").on_hover_text("หน้าแรก (lumra://home)").clicked() {
            action.go_home = true;
        }

        ui.separator();

        // Address Bar
        let response = ui.add_sized(
            [ui.available_width() - 170.0, 24.0],
            egui::TextEdit::singleline(url_input)
                .hint_text("พิมพ์ URL (เช่น lumra://home หรือ https://...) แล้วกด Enter"),
        );

        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            action.navigate_to = Some(url_input.clone());
        }

        ui.separator();

        // Zoom Controls
        if ui.small_button("-").on_hover_text("ซูมออก").clicked() {
            action.zoom_out = true;
        }
        let zoom_pct = format!("{:.0}%", zoom * 100.0);
        if ui.small_button(&zoom_pct).on_hover_text("รีเซ็ตซูม 100%").clicked() {
            action.zoom_reset = true;
        }
        if ui.small_button("+").on_hover_text("ซูมเข้า").clicked() {
            action.zoom_in = true;
        }

        response
    }).inner
}
