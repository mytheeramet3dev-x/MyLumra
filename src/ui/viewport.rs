use egui::{Color32, Key, Rect, Rounding, ScrollArea, Stroke, TextureHandle, Ui, Vec2};
use crate::engine::PageRender;

#[derive(Default)]
pub struct FindState {
    pub is_open: bool,
    pub query: String,
    pub current_match: usize,
    pub total_matches: usize,
}

#[derive(Default)]
pub struct SelectionState {
    pub selected_text: String,
    pub drag_start: Option<egui::Pos2>,
    pub drag_current: Option<egui::Pos2>,
    pub copied_notification: bool,
}

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
    web_mode: bool,
    view_source: bool,
    raw_source: &str,
    find_state: &mut FindState,
    selection_state: &mut SelectionState,
) -> ViewportOutcome {
    let mut outcome = ViewportOutcome {
        clicked_link: None,
        hovered_link: None,
    };

    // 1. View Source Mode
    if view_source {
        render_source_viewer(ui, raw_source);
        return outcome;
    }

    // 2. Error Display
    if let Some(err) = error_message {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(egui::RichText::new("⚠️ ไม่สามารถโหลดหน้าเว็บได้").color(Color32::from_rgb(220, 50, 50)));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(err).color(Color32::from_rgb(180, 40, 40)));
        });
        return outcome;
    }

    // 3. Loading Indicator
    if is_loading && textures.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.spinner();
            ui.add_space(10.0);
            ui.label("กำลังดาวน์โหลดและประมวลผล Typst...");
        });
        return outcome;
    }

    // Handle Keyboard Copy Shortcut (Cmd+C / Ctrl+C)
    if ui.input(|i| i.modifiers.command && i.key_pressed(Key::C)) && !selection_state.selected_text.is_empty() {
        ui.output_mut(|o| o.copied_text = selection_state.selected_text.clone());
        selection_state.copied_notification = true;
    }

    // Reset match count for this frame
    let query_clean = find_state.query.trim().to_lowercase();
    let mut current_match_counter = 0;

    let bg_fill = if web_mode {
        Color32::WHITE
    } else {
        Color32::from_rgb(226, 232, 240)
    };

    // Detect mouse selection dragging
    let pointer = ui.input(|i| i.pointer.clone());
    if pointer.primary_down() {
        if selection_state.drag_start.is_none() {
            selection_state.drag_start = pointer.interact_pos();
        }
        selection_state.drag_current = pointer.interact_pos();
    } else if pointer.primary_released() {
        // Drag finished, keep selection active until next click
    }

    let selection_rect = match (selection_state.drag_start, selection_state.drag_current) {
        (Some(start), Some(curr)) if start.distance(curr) > 8.0 => {
            Some(Rect::from_two_pos(start, curr))
        }
        _ => None,
    };

    let mut newly_selected_spans = Vec::new();

    egui::Frame::none()
        .fill(bg_fill)
        .show(ui, |ui| {
            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        if !web_mode {
                            ui.add_space(15.0);
                        }

                        for (idx, texture) in textures.iter().enumerate() {
                            let page_data = pages.get(idx);
                            let display_size = Vec2::new(
                                texture.size()[0] as f32 * zoom,
                                texture.size()[1] as f32 * zoom,
                            );

                            let (rect, response) = ui.allocate_exact_size(display_size, egui::Sense::click_and_drag());

                            if !web_mode {
                                // Draw paper shadow in doc mode
                                let shadow_rect = rect.expand(2.0);
                                ui.painter().rect_filled(
                                    shadow_rect,
                                    Rounding::same(4.0),
                                    Color32::from_black_alpha(25),
                                );
                            }

                            ui.painter().rect_filled(
                                rect,
                                Rounding::same(if web_mode { 0.0 } else { 2.0 }),
                                Color32::WHITE,
                            );

                            // Paint page image
                            ui.painter().image(
                                texture.id(),
                                rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                Color32::WHITE,
                            );

                            if !web_mode {
                                ui.painter().rect_stroke(
                                    rect,
                                    Rounding::same(2.0),
                                    Stroke::new(1.0_f32, Color32::from_gray(210)),
                                );
                            }

                            if let Some(page) = page_data {
                                // 1. Text Selection & Search Highlights
                                for span in &page.text_spans {
                                    let span_x = rect.min.x + (span.rect.min.x * zoom);
                                    let span_y = rect.min.y + (span.rect.min.y * zoom);
                                    let span_w = span.rect.width() * zoom;
                                    let span_h = span.rect.height() * zoom;

                                    let span_screen_rect = Rect::from_min_size(
                                        egui::pos2(span_x, span_y),
                                        egui::vec2(span_w, span_h),
                                    );

                                    // Check Search Highlight
                                    if !query_clean.is_empty() && span.text.to_lowercase().contains(&query_clean) {
                                        current_match_counter += 1;
                                        let is_active = current_match_counter == find_state.current_match;
                                        let hl_color = if is_active {
                                            Color32::from_rgba_unmultiplied(255, 152, 0, 190) // Orange for active
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 235, 59, 130) // Yellow for matches
                                        };

                                        ui.painter().rect_filled(
                                            span_screen_rect.expand(1.0),
                                            Rounding::same(2.0),
                                            hl_color,
                                        );
                                    }

                                    // Check Selection
                                    if let Some(sel) = selection_rect {
                                        if sel.intersects(span_screen_rect) {
                                            ui.painter().rect_filled(
                                                span_screen_rect,
                                                Rounding::same(1.0),
                                                Color32::from_rgba_unmultiplied(59, 130, 246, 90),
                                            );
                                            newly_selected_spans.push((span_screen_rect.min.y, span_screen_rect.min.x, span.text.clone()));
                                        }
                                    }
                                }

                                // 2. Check mouse interaction with links on this page
                                if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
                                    if rect.contains(hover_pos) {
                                        for link in &page.links {
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

                                                // Highlight link on hover
                                                ui.painter().rect_stroke(
                                                    link_screen_rect.expand(1.0),
                                                    Rounding::same(2.0),
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

                            if !web_mode {
                                ui.add_space(20.0);
                            }
                        }

                        if !web_mode {
                            ui.add_space(20.0);
                        }
                    });
                });
        });

    // Update selection text if new selection was made
    if !newly_selected_spans.is_empty() {
        // Sort spans in reading order (top-to-bottom, left-to-right)
        newly_selected_spans.sort_by(|a, b| {
            let y_diff = (a.0 - b.0).abs();
            if y_diff < 10.0 {
                a.1.partial_cmp(&b.1).unwrap()
            } else {
                a.0.partial_cmp(&b.0).unwrap()
            }
        });
        selection_state.selected_text = newly_selected_spans
            .into_iter()
            .map(|(_, _, t)| t)
            .collect::<Vec<_>>()
            .join(" ");
    }

    find_state.total_matches = current_match_counter;

    // 4. Render Floating Find Bar (Top-Right)
    if find_state.is_open {
        render_find_bar(ui, find_state);
    }

    // 5. Floating Copy Notification Pill
    if !selection_state.selected_text.is_empty() {
        render_copy_pill(ui, selection_state);
    }

    outcome
}

fn render_find_bar(ui: &mut Ui, find_state: &mut FindState) {
    let screen = ui.ctx().screen_rect();
    let find_rect = Rect::from_min_size(
        egui::pos2(screen.max.x - 340.0, screen.min.y + 70.0),
        Vec2::new(320.0, 38.0),
    );

    egui::Area::new(egui::Id::new("find_bar_area"))
        .fixed_pos(find_rect.min)
        .show(ui.ctx(), |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgb(255, 255, 255))
                .rounding(Rounding::same(8.0))
                .stroke(Stroke::new(1.0_f32, Color32::from_gray(190)))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let input = ui.add(
                            egui::TextEdit::singleline(&mut find_state.query)
                                .desired_width(140.0)
                                .hint_text("ค้นหาในหน้า..."),
                        );
                        if find_state.query.is_empty() {
                            input.request_focus();
                        }

                        // Match count
                        if find_state.total_matches > 0 {
                            if find_state.current_match == 0 {
                                find_state.current_match = 1;
                            }
                            ui.label(format!("{}/{}", find_state.current_match, find_state.total_matches));
                        } else if !find_state.query.trim().is_empty() {
                            ui.label(egui::RichText::new("0/0").color(Color32::RED));
                        }

                        // Prev match
                        if ui.small_button("▲").on_hover_text("ผลลัพธ์ก่อนหน้า").clicked() {
                            if find_state.total_matches > 0 {
                                if find_state.current_match <= 1 {
                                    find_state.current_match = find_state.total_matches;
                                } else {
                                    find_state.current_match -= 1;
                                }
                            }
                        }

                        // Next match
                        if ui.small_button("▼").on_hover_text("ผลลัพธ์ถัดไป").clicked() {
                            if find_state.total_matches > 0 {
                                if find_state.current_match >= find_state.total_matches {
                                    find_state.current_match = 1;
                                } else {
                                    find_state.current_match += 1;
                                }
                            }
                        }

                        // Close button
                        if ui.small_button("✕").on_hover_text("ปิด (Esc)").clicked() {
                            find_state.is_open = false;
                        }
                    });
                });
        });
}

fn render_copy_pill(ui: &mut Ui, selection_state: &mut SelectionState) {
    let screen = ui.ctx().screen_rect();
    let pill_rect = Rect::from_min_size(
        egui::pos2(screen.max.x - 260.0, screen.max.y - 60.0),
        Vec2::new(240.0, 36.0),
    );

    egui::Area::new(egui::Id::new("copy_pill_area"))
        .fixed_pos(pill_rect.min)
        .show(ui.ctx(), |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgb(30, 41, 59))
                .rounding(Rounding::same(18.0))
                .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let btn_text = if selection_state.copied_notification {
                            "✅ คัดลอกเรียบร้อยแล้ว!"
                        } else {
                            "📋 คัดลอกข้อความ (Cmd+C)"
                        };
                        if ui.button(egui::RichText::new(btn_text).color(Color32::WHITE).strong()).clicked() {
                            ui.output_mut(|o| o.copied_text = selection_state.selected_text.clone());
                            selection_state.copied_notification = true;
                        }

                        if ui.button(egui::RichText::new("✕").color(Color32::from_gray(200))).clicked() {
                            selection_state.selected_text.clear();
                            selection_state.drag_start = None;
                            selection_state.drag_current = None;
                            selection_state.copied_notification = false;
                        }
                    });
                });
        });
}

fn render_source_viewer(ui: &mut Ui, raw_source: &str) {
    ui.vertical(|ui| {
        // Top Toolbar
        egui::Frame::none()
            .fill(Color32::from_rgb(241, 245, 249))
            .inner_margin(egui::Margin::symmetric(14.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "📄 ซอร์สโค้ด Typst ({} บรรทัด • {} ไบต์)",
                            raw_source.lines().count(),
                            raw_source.len()
                        ))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("📋 คัดลอกโค้ดทั้งหมด").clicked() {
                            ui.output_mut(|o| o.copied_text = raw_source.to_string());
                        }
                    });
                });
            });

        // Code Content with Line Numbers
        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::none()
                    .fill(Color32::from_rgb(248, 250, 252))
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let mut numbered_code = String::new();
                        for (i, line) in raw_source.lines().enumerate() {
                            numbered_code.push_str(&format!("{:4} │ {}\n", i + 1, line));
                        }

                        ui.add(
                            egui::TextEdit::multiline(&mut numbered_code.as_str())
                                .font(egui::TextStyle::Monospace)
                                .text_color(Color32::from_rgb(30, 41, 59))
                                .desired_width(f32::INFINITY),
                        );
                    });
            });
    });
}
