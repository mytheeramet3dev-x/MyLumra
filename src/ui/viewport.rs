use egui::{Color32, Rect, ScrollArea, Stroke, TextureHandle, Ui, Vec2};
use crate::engine::PageRender;

pub struct ViewportOutcome {
    pub clicked_link: Option<String>,
    pub hovered_link: Option<String>,
}

pub fn render_viewport(
    ui: &mut Ui,
    pages: &[PageRender],
    textures: &[TextureHandle],
    is_loading: bool,
    error_message: Option<&str>,
    zoom: f32,
) -> ViewportOutcome {
    let mut outcome = ViewportOutcome {
        clicked_link: None,
        hovered_link: None,
    };

    if let Some(err) = error_message {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(egui::RichText::new("⚠️ ไม่สามารถโหลดหน้าเว็บได้").color(Color32::from_rgb(220, 50, 50)));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(err).color(Color32::from_rgb(180, 40, 40)));
        });
        return outcome;
    }

    if is_loading && textures.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.spinner();
            ui.add_space(10.0);
            ui.label("กำลังดาวน์โหลดและประมวลผล Typst...");
        });
        return outcome;
    }

    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(15.0);

                for (idx, texture) in textures.iter().enumerate() {
                    let page_data = pages.get(idx);
                    let display_size = Vec2::new(
                        texture.size()[0] as f32 * zoom,
                        texture.size()[1] as f32 * zoom,
                    );

                    // Document shadow and border frame
                    let (rect, response) = ui.allocate_exact_size(display_size, egui::Sense::click_and_drag());

                    // Draw paper shadow and white background
                    let shadow_rect = rect.expand(2.0);
                    ui.painter().rect_filled(
                        shadow_rect,
                        egui::Rounding::same(4.0),
                        Color32::from_black_alpha(25),
                    );
                    ui.painter().rect_filled(
                        rect,
                        egui::Rounding::same(2.0),
                        Color32::WHITE,
                    );

                    // Paint texture onto the rect
                    ui.painter().image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );

                    // Draw clean page boundary
                    ui.painter().rect_stroke(
                        rect,
                        egui::Rounding::same(2.0),
                        Stroke::new(1.0_f32, Color32::from_gray(210)),
                    );

                    // Check mouse interaction with links on this page
                    if let Some(page) = page_data {
                        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                            if rect.contains(hover_pos) {
                                for link in &page.links {
                                    // Scale link rect by zoom factor
                                    let link_x = rect.min.x + (link.rect.min.x * zoom);
                                    let link_y = rect.min.y + (link.rect.min.y * zoom);
                                    let link_w = link.rect.width() * zoom;
                                    let link_h = link.rect.height() * zoom;

                                    let link_screen_rect = Rect::from_min_size(
                                        egui::pos2(link_x, link_y),
                                        egui::vec2(link_w, link_h),
                                    );

                                    if link_screen_rect.contains(hover_pos) {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        outcome.hovered_link = Some(link.target.clone());

                                        // Highlight link on hover with subtle outline
                                        ui.painter().rect_stroke(
                                            link_screen_rect.expand(1.0),
                                            egui::Rounding::same(2.0),
                                            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(66, 153, 225, 120)),
                                        );

                                        if response.clicked() {
                                            outcome.clicked_link = Some(link.target.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    ui.add_space(20.0);
                }
            });
        });

    outcome
}
