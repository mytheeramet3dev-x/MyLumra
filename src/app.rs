use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use eframe::egui::{self, Color32, RichText};
use crate::engine::{compile_and_render, DocumentModel, FontStore, LumraAction, PageRender};
use crate::net::ContentFetcher;
use crate::ui::{
    render_bookmarks_bar, render_navbar, render_tabs, render_viewport, Bookmark, FindState,
    NavbarAction, SelectionState,
};

pub struct NavigationResult {
    pub tab_id: usize,
    /// Monotonically increasing per-tab generation; stale results are discarded.
    pub request_id: u64,
    pub url: String,
    pub raw_source: String,
    pub result: anyhow::Result<Vec<PageRender>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationState {
    Loading { request_id: u64 },
    Ready,
    Failed { message: String },
}

impl NavigationState {
    fn is_loading(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    fn matches_request(&self, request_id: u64) -> bool {
        matches!(self, Self::Loading { request_id: current } if *current == request_id)
    }

    fn error_message(&self) -> Option<&str> {
        match self {
            Self::Failed { message } => Some(message),
            _ => None,
        }
    }
}

pub struct TabState {
    pub id: usize,
    /// Identifies the newest navigation or document compilation requested by this tab.
    pub request_generation: u64,
    pub title: String,
    pub url_input: String,
    pub current_url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub pages: Vec<PageRender>,
    /// Structured metadata and indexes for the currently committed Typst document.
    pub document: Option<DocumentModel>,
    pub textures: Vec<egui::TextureHandle>,
    pub navigation_state: NavigationState,
    pub zoom: f32,
    pub web_mode: bool,
    pub view_source: bool,
    pub raw_source: String,
    pub find_state: FindState,
    pub selection_state: SelectionState,
    pub state: HashMap<String, String>,
    pub last_file_mod: Option<std::time::SystemTime>,
    #[allow(dead_code)]
    pub last_etag: Option<String>,
    pub last_check_time: std::time::Instant,
}

impl TabState {
    pub fn new(id: usize, initial_url: &str) -> Self {
        Self {
            id,
            request_generation: 0,
            title: extract_tab_title(initial_url),
            url_input: initial_url.to_string(),
            current_url: initial_url.to_string(),
            history: vec![initial_url.to_string()],
            history_index: 0,
            pages: Vec::new(),
            document: None,
            textures: Vec::new(),
            navigation_state: NavigationState::Loading { request_id: 0 },
            zoom: 1.0,
            web_mode: true,
            view_source: false,
            raw_source: String::new(),
            find_state: FindState::default(),
            selection_state: SelectionState::default(),
            state: HashMap::new(),
            last_file_mod: None,
            last_etag: None,
            last_check_time: std::time::Instant::now(),
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
    last_viewport_size: [f32; 2],
    bookmarks: Vec<Bookmark>,
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
        let mut initial_tab = TabState::new(0, initial_url);
        initial_tab.request_generation = 1;
        initial_tab.navigation_state = NavigationState::Loading { request_id: 1 };

        let default_bookmarks = vec![
            Bookmark {
                title: "🏠 Home".to_string(),
                url: "lumra://home".to_string(),
            },
            Bookmark {
                title: "⚡ Counter (Interactive)".to_string(),
                url: "file:///Users/home/Desktop/project/lumra-web/examples/counter.typ".to_string(),
            },
            Bookmark {
                title: "🌐 Lumra-Web".to_string(),
                url: "file:///Users/home/Desktop/project/lumra-web/examples/landing.typ".to_string(),
            },
            Bookmark {
                title: "📖 Blog & Math".to_string(),
                url: "file:///Users/home/Desktop/project/lumra-web/examples/blog.typ".to_string(),
            },
            Bookmark {
                title: "🧩 UI Kit".to_string(),
                url: "file:///Users/home/Desktop/project/lumra-web/examples/components_gallery.typ".to_string(),
            },
            Bookmark {
                title: "📖 Cheatsheet".to_string(),
                url: "lumra://cheatsheet".to_string(),
            },
        ];

        let app = Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            font_store,
            fetcher,
            tx,
            rx,
            hovered_url: None,
            next_tab_id: 1,
            last_viewport_size: [900.0, 700.0],
            bookmarks: default_bookmarks,
        };

        app.trigger_navigation(0, 1, initial_url.to_string(), HashMap::new());
        app
    }

    fn trigger_navigation(
        &self,
        tab_id: usize,
        request_id: u64,
        url: String,
        state: HashMap<String, String>,
    ) {
        let font_store = self.font_store.clone();
        let fetcher = self.fetcher.clone();
        let tx = self.tx.clone();
        let [vp_w, vp_h] = self.last_viewport_size;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();

            let content_res = match rt {
                Ok(runtime) => runtime.block_on(fetcher.fetch(&url)),
                Err(e) => Err(anyhow::anyhow!("Runtime error: {}", e)),
            };

            let raw_source = content_res.as_ref().ok().cloned().unwrap_or_default();

            let compile_res = match &content_res {
                Ok(source) => compile_and_render(
                    font_store,
                    fetcher,
                    &url,
                    source,
                    vp_w,
                    vp_h,
                    2.0,
                    &state,
                ),
                Err(e) => Err(anyhow::anyhow!("{}", e)),
            };

            let _ = tx.send(NavigationResult {
                tab_id,
                request_id,
                url,
                raw_source,
                result: compile_res,
            });
        });
    }

    fn dispatch_action(&mut self, action_url: &str) {
        if let Some(act) = LumraAction::parse(action_url) {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                act.apply(&mut tab.state);

                // A new action compilation supersedes any older compilation for this tab.
                tab.request_generation = tab.request_generation.wrapping_add(1);
                tab.navigation_state = NavigationState::Loading {
                request_id: tab.request_generation,
            };
                let request_id = tab.request_generation;

                // Fast in-memory re-compilation with updated state
                let source = tab.raw_source.clone();
                let url = tab.current_url.clone();
                let tab_id = tab.id;
                let state = tab.state.clone();
                let font_store = self.font_store.clone();
                let fetcher = self.fetcher.clone();
                let [vp_w, vp_h] = self.last_viewport_size;
                let tx = self.tx.clone();

                std::thread::spawn(move || {
                    let compile_res = compile_and_render(
                        font_store,
                        fetcher,
                        &url,
                        &source,
                        vp_w,
                        vp_h,
                        2.0,
                        &state,
                    );
                    let _ = tx.send(NavigationResult {
                        tab_id,
                        request_id,
                        url,
                        raw_source: source,
                        result: compile_res,
                    });
                });
            }
        }
    }

    fn navigate(&mut self, url: String) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.url_input = url.clone();
            tab.view_source = false;

            // Reset tab reactive state on new navigation
            tab.state.clear();

            // Append to history if it's a new location
            if tab.history.get(tab.history_index) != Some(&url) {
                tab.history.truncate(tab.history_index + 1);
                tab.history.push(url.clone());
                tab.history_index = tab.history.len() - 1;
            }

            tab.request_generation = tab.request_generation.wrapping_add(1);

            tab.navigation_state = NavigationState::Loading { request_id: tab.request_generation };
            let tab_id = tab.id;
            let request_id = tab.request_generation;
            let tab_state = tab.state.clone();
            self.trigger_navigation(tab_id, request_id, url, tab_state);
        }
    }

    fn go_back(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.history_index > 0 {
                tab.history_index -= 1;
                let prev_url = tab.history[tab.history_index].clone();
                tab.url_input = prev_url.clone();
                tab.view_source = false;
                tab.state.clear();
                tab.request_generation = tab.request_generation.wrapping_add(1);
                tab.navigation_state = NavigationState::Loading {
                    request_id: tab.request_generation,
                };
                let tab_id = tab.id;
                let request_id = tab.request_generation;
                self.trigger_navigation(tab_id, request_id, prev_url, HashMap::new());
            }
        }
    }

    fn go_forward(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.history_index + 1 < tab.history.len() {
                tab.history_index += 1;
                let next_url = tab.history[tab.history_index].clone();
                    tab.url_input = next_url.clone();
                tab.view_source = false;
                tab.state.clear();
                tab.request_generation = tab.request_generation.wrapping_add(1);
                tab.navigation_state = NavigationState::Loading { request_id: tab.request_generation };
                let tab_id = tab.id;
                let request_id = tab.request_generation;
                self.trigger_navigation(tab_id, request_id, next_url, HashMap::new());
            }
        }
    }

    fn reload(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            let current = tab.current_url.clone();
            tab.request_generation = tab.request_generation.wrapping_add(1);
            tab.navigation_state = NavigationState::Loading { request_id: tab.request_generation };
            let tab_id = tab.id;
            let request_id = tab.request_generation;
            let tab_state = tab.state.clone();
            self.trigger_navigation(tab_id, request_id, current, tab_state);
        }
    }

    fn open_new_tab(&mut self, url: &str) {
        let new_id = self.next_tab_id;
        self.next_tab_id += 1;

        let mut new_tab = TabState::new(new_id, url);
        new_tab.request_generation = 1;
        new_tab.navigation_state = NavigationState::Loading { request_id: 1 };
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.trigger_navigation(new_id, 1, url.to_string(), HashMap::new());
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
        // Track viewport size for responsive layout
        let screen = ctx.screen_rect();
        self.last_viewport_size = [screen.width(), screen.height()];

        // 0. Live Reload check for local development (Milestone 2)
        let mut need_reload = false;
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if tab.last_check_time.elapsed() >= std::time::Duration::from_millis(250) {
                tab.last_check_time = std::time::Instant::now();

                if tab.current_url.starts_with("file://") && !tab.navigation_state.is_loading() {
                    let path_str = tab.current_url.trim_start_matches("file://");
                    if let Ok(meta) = std::fs::metadata(path_str) {
                        if let Ok(mod_time) = meta.modified() {
                            if let Some(prev) = tab.last_file_mod {
                                if mod_time > prev {
                                    tab.last_file_mod = Some(mod_time);
                                    need_reload = true;
                                }
                            } else {
                                tab.last_file_mod = Some(mod_time);
                            }
                        }
                    }
                }
            }
        }
        if need_reload {
            self.reload();
        }

        // 1. Process async navigation results
        while let Ok(nav) = self.rx.try_recv() {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == nav.tab_id) {
                // Ignore results from superseded requests and compilations.
                if !is_current_request(tab.request_generation, nav.request_id)
                    || !tab.navigation_state.matches_request(nav.request_id)
                {
                    continue;
                }

                tab.raw_source = nav.raw_source;
                match nav.result {
                    Ok(pages) => {
                        tab.document = Some(DocumentModel::from_rendered(
                            &nav.url,
                            &tab.raw_source,
                            &pages,
                        ));
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
                        tab.title = tab.document.as_ref()
                            .map(|document| document.metadata.title.clone())
                            .unwrap_or_else(|| extract_tab_title(&nav.url));
                        tab.navigation_state = NavigationState::Ready;
                    }
                    Err(e) => {
                        tab.navigation_state = NavigationState::Failed { message: e.to_string() };
                    }
                }
                ctx.request_repaint();
            }
        }

        // 2. Render Top Panel (Tabs + Navbar + Bookmarks Bar)
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
                    let is_bm = self.bookmarks.iter().any(|b| b.url == tab.current_url);

                    render_navbar(
                        ui,
                        &mut tab.url_input,
                        can_back,
                        can_forward,
                        tab.zoom,
                        tab.web_mode,
                        tab.view_source,
                        is_bm,
                        tab.navigation_state.is_loading(),
                        &mut action,
                    );

                    ui.add_space(3.0);
                    // Bookmarks Bar
                    render_bookmarks_bar(ui, &self.bookmarks, &mut action);
                }
            });

        // 3. Render Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none().fill(Color32::from_rgb(240, 244, 248)).inner_margin(egui::Margin::symmetric(10.0, 4.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(tab) = self.tabs.get(self.active_tab) {
                        if tab.navigation_state.is_loading() {
                            ui.label(RichText::new("กำลังโหลด...").size(12.0).color(Color32::from_rgb(100, 100, 100)));
                        } else if let Some(ref hovered) = self.hovered_url {
                            ui.label(RichText::new(hovered).size(12.0).color(Color32::from_rgb(40, 90, 180)));
                        } else {
                            let mode_desc = if tab.view_source {
                                "Source Code View"
                            } else if tab.web_mode {
                                "Web Canvas"
                            } else {
                                "Print View"
                            };
                            ui.label(RichText::new(format!("Lumra 0.2.0 • {} หน้า • {}", tab.pages.len(), mode_desc)).size(12.0).color(Color32::from_rgb(120, 120, 120)));
                        }
                    }
                });
            });

        // 4. Render Main Content (Viewport)
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(226, 232, 240)))
            .show(ctx, |ui| {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    let outcome = render_viewport(
                        ui,
                        &tab.pages,
                        &tab.textures,
                        tab.navigation_state.is_loading(),
                        tab.navigation_state.error_message(),
                        tab.zoom,
                        tab.web_mode,
                        tab.view_source,
                        &tab.raw_source,
                        &mut tab.find_state,
                        &mut tab.selection_state,
                    );

                    self.hovered_url = outcome.hovered_link;

                    if let Some(clicked) = outcome.clicked_link {
                        action.navigate_to = Some(clicked);
                    }
                }
            });

        // 5. Handle User Actions
        if let Some(target) = action.navigate_to {
            if target.starts_with("lumra-action://") {
                self.dispatch_action(&target);
            } else {
                self.navigate(target);
            }
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
        if action.toggle_web_mode {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.web_mode = !tab.web_mode;
            }
        }
        if action.toggle_view_source {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.view_source = !tab.view_source;
            }
        }
        if action.toggle_find {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                tab.find_state.is_open = !tab.find_state.is_open;
            }
        }
        if action.toggle_bookmark {
            if let Some(tab) = self.tabs.get(self.active_tab) {
                let cur_url = tab.current_url.clone();
                let cur_title = tab.title.clone();
                if let Some(pos) = self.bookmarks.iter().position(|b| b.url == cur_url) {
                    self.bookmarks.remove(pos);
                } else {
                    self.bookmarks.push(Bookmark {
                        title: cur_title,
                        url: cur_url,
                    });
                }
            }
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

fn is_current_request(current_generation: u64, result_generation: u64) -> bool {
    current_generation == result_generation
}

#[cfg(test)]
mod navigation_tests {
    use super::{is_current_request, NavigationState};

    #[test]
    fn accepts_only_the_latest_generation() {
        assert!(is_current_request(7, 7));
        assert!(!is_current_request(8, 7));
        assert!(!is_current_request(7, 8));
    }

    #[test]
    fn navigation_state_reports_loading_and_failure() {
        let loading = NavigationState::Loading { request_id: 12 };
        assert!(loading.is_loading());
        assert!(loading.matches_request(12));
        assert!(!loading.matches_request(11));
        assert_eq!(loading.error_message(), None);

        let ready = NavigationState::Ready;
        assert!(!ready.is_loading());
        assert_eq!(ready.error_message(), None);

        let failed = NavigationState::Failed {
            message: "Typst compilation failed".to_string(),
        };
        assert!(!failed.is_loading());
        assert_eq!(failed.error_message(), Some("Typst compilation failed"));
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
