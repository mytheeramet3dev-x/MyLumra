use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use eframe::egui::{self, Color32, RichText};
use crate::engine::{compile_and_render, FontStore, PageRender};
use crate::net::ContentFetcher;
use crate::ui::{render_navbar, render_tabs, render_viewport, NavbarAction};

pub struct NavigationResult {
    pub tab_id: usize,
    pub url: String,
    pub result: anyhow::Result<Vec<PageRender>>,
}

pub struct TabState {
    pub id: usize,
    pub title: String,
    pub url_input: String,
    pub current_url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub pages: Vec<PageRender>,
    pub textures: Vec<egui::TextureHandle>,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub zoom: f32,
}

impl TabState {
    pub fn new(id: usize, initial_url: &str) -> Self {
        Self {
            id,
            title: extract_tab_title(initial_url),
            url_input: initial_url.to_string(),
            current_url: initial_url.to_string(),
            history: vec![initial_url.to_string()],
            history_index: 0,
            pages: Vec::new(),
            textures: Vec::new(),
            is_loading: true,
            error_message: None,
            zoom: 1.0,
        }
    }
}

pub struct LumraApp {
    tabs: Vec<TabState>,
    active_tab: usize,
    font_store: Arc<FontStore>,
    fetcher: Arc<ContentFetcher>,
    tx: Sender<NavigationResult>,
    rx: Receiver<NavigationResult>,
    hovered_url: Option<String>,
    next_tab_id: usize,
}

impl LumraApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Setup font definitions with Thai font fallback for UI
        let mut fonts = egui::FontDefinitions::default();
        let thai_font_paths = [
            "/System/Library/Fonts/Supplemental/Thonburi.ttc",
            "/System/Library/Fonts/ThonburiUI.ttc",
            "/System/Library/Fonts/Supplemental/Ayuthaya.ttf",
            "/System/Library/Fonts/Supplemental/SukhumvitSet.ttc",
        ];
        for path in thai_font_paths {
            if let Ok(font_bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    "system_thai".to_owned(),
                    egui::FontData::from_owned(font_bytes),
                );
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .push("system_thai".to_owned());
                fonts
                    .families
                    .entry(egui::FontFamily::Monospace)
                    .or_default()
                    .push("system_thai".to_owned());
                break;
            }
        }
        cc.egui_ctx.set_fonts(fonts);

        // Set up nice visuals & typography
        let mut visuals = egui::Visuals::light();
        visuals.faint_bg_color = Color32::from_rgb(235, 240, 246);
        visuals.window_fill = Color32::from_rgb(248, 250, 252);
        cc.egui_ctx.set_visuals(visuals);

        let font_store = Arc::new(FontStore::init());
        let fetcher = Arc::new(ContentFetcher::new());
        let (tx, rx) = channel();

        let initial_url = "lumra://home";
        let initial_tab = TabState::new(0, initial_url);

        let app = Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            font_store,
            fetcher,
            tx,
            rx,
            hovered_url: None,
            next_tab_id: 1,
        };

        app.trigger_navigation(0, initial_url.to_string());
        app
    }

    fn trigger_navigation(&self, tab_id: usize, url: String) {
        let font_store = self.font_store.clone();
        let fetcher = self.fetcher.clone();
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            let content_res = match rt {
                Ok(runtime) => runtime.block_on(fetcher.fetch(&url)),
                Err(e) => Err(anyhow::anyhow!("Runtime error: {}", e)),
            };

            let compile_res = match content_res {
                Ok(source) => compile_and_render(font_store, &source, 2.0),
                Err(e) => Err(e),
            };

            let _ = tx.send(NavigationResult {
                tab_id,
                url,
                result: compile_res,
            });
        });
    }

    fn navigate(&mut self, url: String) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.is_loading = true;
            tab.error_message = None;
            tab.url_input = url.clone();

            // Append to history if it's a new location
            if tab.history.get(tab.history_index) != Some(&url) {
                tab.history.truncate(tab.history_index + 1);
                tab.history.push(url.clone());
                tab.history_index = tab.history.len() - 1;
            }

            let tab_id = tab.id;
            self.trigger_navigation(tab_id, url);
        }
    }

    fn go_back(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.history_index > 0 {
                tab.history_index -= 1;
                let prev_url = tab.history[tab.history_index].clone();
                tab.is_loading = true;
                tab.url_input = prev_url.clone();
                let tab_id = tab.id;
                self.trigger_navigation(tab_id, prev_url);
            }
        }
    }

    fn go_forward(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.history_index + 1 < tab.history.len() {
                tab.history_index += 1;
                let next_url = tab.history[tab.history_index].clone();
                tab.is_loading = true;
                tab.url_input = next_url.clone();
                let tab_id = tab.id;
                self.trigger_navigation(tab_id, next_url);
            }
        }
    }

    fn reload(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            let current = tab.current_url.clone();
            tab.is_loading = true;
            let tab_id = tab.id;
            self.trigger_navigation(tab_id, current);
        }
    }

    fn open_new_tab(&mut self, url: &str) {
        let new_id = self.next_tab_id;
        self.next_tab_id += 1;

        let new_tab = TabState::new(new_id, url);
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.trigger_navigation(new_id, url.to_string());
    }

    fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() > 1 && idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }
}

impl eframe::App for LumraApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Process async navigation results
        while let Ok(nav) = self.rx.try_recv() {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == nav.tab_id) {
                tab.is_loading = false;
                match nav.result {
                    Ok(pages) => {
                        tab.textures.clear();
                        for (idx, page) in pages.iter().enumerate() {
                            let tex = ctx.load_texture(
                                format!("tab_{}_page_{}", nav.tab_id, idx),
                                page.image.clone(),
                                Default::default(),
                            );
                            tab.textures.push(tex);
                        }
                        tab.pages = pages;
                        tab.current_url = nav.url.clone();
                        tab.url_input = nav.url.clone();
                        tab.title = extract_tab_title(&nav.url);
                        tab.error_message = None;
                    }
                    Err(e) => {
                        tab.error_message = Some(e.to_string());
                    }
                }
                ctx.request_repaint();
            }
        }

        // 2. Render Top Panel (Tabs + Navbar)
        let mut action = NavbarAction::default();

        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(240, 244, 248)).inner_margin(6.0))
            .show(ctx, |ui| {
                // Tab Bar
                let tab_titles: Vec<String> = self.tabs.iter().map(|t| t.title.clone()).collect();
                render_tabs(ui, &tab_titles, self.active_tab, &mut action);

                ui.add_space(4.0);

                // Navbar
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    let can_back = tab.history_index > 0;
                    let can_forward = tab.history_index + 1 < tab.history.len();
                    render_navbar(
                        ui,
                        &mut tab.url_input,
                        can_back,
                        can_forward,
                        tab.zoom,
                        tab.is_loading,
                        &mut action,
                    );
                }
            });

        // 3. Render Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgb(240, 244, 248)).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(tab) = self.tabs.get(self.active_tab) {
                        if tab.is_loading {
                            ui.label(RichText::new("กำลังโหลด...").size(12.0).color(Color32::from_rgb(100, 100, 100)));
                        } else if let Some(ref hovered) = self.hovered_url {
                            ui.label(RichText::new(hovered).size(12.0).color(Color32::from_rgb(40, 90, 180)));
                        } else {
                            ui.label(RichText::new(format!("Lumra 0.2.0 • {} หน้า", tab.pages.len())).size(12.0).color(Color32::from_rgb(120, 120, 120)));
                        }
                    }
                });
            });

        // 4. Render Main Content (Viewport)
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(226, 232, 240)))
            .show(ctx, |ui| {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    let outcome = render_viewport(
                        ui,
                        &tab.pages,
                        &tab.textures,
                        tab.is_loading,
                        tab.error_message.as_deref(),
                        tab.zoom,
                    );

                    self.hovered_url = outcome.hovered_link;

                    if let Some(clicked) = outcome.clicked_link {
                        action.navigate_to = Some(clicked);
                    }
                }
            });

        // 5. Handle User Actions
        if let Some(target) = action.navigate_to {
            self.navigate(target);
        }
        if action.go_back {
            self.go_back();
        }
        if action.go_forward {
            self.go_forward();
        }
        if action.reload {
            self.reload();
        }
        if action.go_home {
            self.navigate("lumra://home".to_string());
        }
        if action.zoom_in {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.zoom = (tab.zoom + 0.1).min(3.0);
            }
        }
        if action.zoom_out {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.zoom = (tab.zoom - 0.1).max(0.4);
            }
        }
        if action.zoom_reset {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.zoom = 1.0;
            }
        }
        if action.new_tab {
            self.open_new_tab("lumra://home");
        }
        if let Some(idx) = action.close_tab {
            self.close_tab(idx);
        }
        if let Some(idx) = action.switch_tab {
            if idx < self.tabs.len() {
                self.active_tab = idx;
            }
        }
    }
}

fn extract_tab_title(url: &str) -> String {
    if let Some(page) = url.strip_prefix("lumra://") {
        match page.trim_end_matches('/') {
            "" | "home" | "welcome" => "Lumra Home".to_string(),
            "about" => "About Lumra".to_string(),
            "cheatsheet" => "Typst Cheatsheet".to_string(),
            other => format!("lumra://{}", other),
        }
    } else if let Some(path) = url.strip_prefix("file://") {
        std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| url.to_string())
    } else {
        url.to_string()
    }
}
