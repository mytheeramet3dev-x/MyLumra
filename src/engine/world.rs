use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{Datelike, Utc};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::net::ContentFetcher;

/// Holds pre-loaded font metadata and font buffers
pub struct FontStore {
    pub book: LazyHash<FontBook>,
    pub fonts: Vec<Font>,
}

impl FontStore {
    /// Load both Typst bundled fonts and system fonts
    pub fn init() -> Self {
        let mut book = FontBook::new();
        let mut fonts = Vec::new();

        // 1. Load bundled fonts from typst-assets
        for font_slice in typst_assets::fonts() {
            let bytes = Bytes::new(font_slice);
            let count = ttf_parser::fonts_in_collection(font_slice).unwrap_or(1);
            for idx in 0..count {
                if let Some(font) = Font::new(bytes.clone(), idx) {
                    book.push(font.info().clone());
                    fonts.push(font);
                }
            }
        }

        // 2. Load system fonts via fontdb
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for face in db.faces() {
            db.with_face_data(face.id, |data, face_index| {
                let bytes = Bytes::new(data.to_vec());
                if let Some(font) = Font::new(bytes, face_index) {
                    book.push(font.info().clone());
                    fonts.push(font);
                }
            });
        }

        tracing::info!("Loaded {} fonts into Lumra FontStore", fonts.len());

        Self {
            book: LazyHash::new(book),
            fonts,
        }
    }
}

/// Custom implementation of `typst::World` for Lumra Browser
pub struct LumraWorld {
    library: LazyHash<Library>,
    fonts: Arc<FontStore>,
    fetcher: Arc<ContentFetcher>,
    base_url: Option<String>,
    main_id: FileId,
    source: Source,
    file_cache: Mutex<HashMap<FileId, Bytes>>,
    source_cache: Mutex<HashMap<FileId, Source>>,
}

impl LumraWorld {
    pub fn new(
        fonts: Arc<FontStore>,
        fetcher: Arc<ContentFetcher>,
        base_url: Option<String>,
        content: String,
        viewport_width: f32,
        viewport_height: f32,
        custom_inputs: &HashMap<String, String>,
    ) -> Self {
        let main_id = FileId::new_fake(VirtualPath::new("index.typ"));
        let source = Source::new(main_id, content);

        let mut inputs = typst::foundations::Dict::new();
        inputs.insert(
            "viewport-width".into(),
            typst::foundations::Value::Str(format!("{:.0}", viewport_width).into()),
        );
        inputs.insert(
            "viewport-height".into(),
            typst::foundations::Value::Str(format!("{:.0}", viewport_height).into()),
        );
        if let Some(ref url) = base_url {
            inputs.insert("url".into(), typst::foundations::Value::Str(url.clone().into()));
        }

        // Inject custom state inputs from Lumra runtime
        for (k, v) in custom_inputs {
            inputs.insert(
                k.as_str().into(),
                typst::foundations::Value::Str(v.as_str().into()),
            );
        }

        let library = Library::builder().with_inputs(inputs).build();

        Self {
            library: LazyHash::new(library),
            fonts,
            fetcher,
            base_url,
            main_id,
            source,
            file_cache: Mutex::new(HashMap::new()),
            source_cache: Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub fn set_file(&self, path: &str, bytes: Bytes) {
        let id = FileId::new_fake(VirtualPath::new(path));
        self.file_cache.lock().unwrap().insert(id, bytes);
    }
}

impl World for LumraWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.fonts.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            return Ok(self.source.clone());
        }

        // Check cache
        {
            let cache = self.source_cache.lock().unwrap();
            if let Some(src) = cache.get(&id) {
                return Ok(src.clone());
            }
        }

        // Resolve path to URL
        let path_str = id.vpath().as_rootless_path().to_string_lossy();
        let resolved_url = self.fetcher.resolve_url(self.base_url.as_deref(), &path_str);

        match self.fetcher.fetch_text_blocking(&resolved_url) {
            Ok(text) => {
                let src = Source::new(id, text);
                self.source_cache.lock().unwrap().insert(id, src.clone());
                Ok(src)
            }
            Err(e) => {
                tracing::error!("Failed to fetch source {}: {}", resolved_url, e);
                Err(FileError::NotFound(std::path::PathBuf::from(path_str.to_string())))
            }
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main_id {
            return Ok(Bytes::new(self.source.text().as_bytes().to_vec()));
        }

        // Check cache
        {
            let cache = self.file_cache.lock().unwrap();
            if let Some(bytes) = cache.get(&id) {
                return Ok(bytes.clone());
            }
        }

        // Resolve path to URL
        let path_str = id.vpath().as_rootless_path().to_string_lossy();
        let resolved_url = self.fetcher.resolve_url(self.base_url.as_deref(), &path_str);

        match self.fetcher.fetch_bytes_blocking(&resolved_url) {
            Ok(raw) => {
                let bytes = Bytes::new(raw.to_vec());
                self.file_cache.lock().unwrap().insert(id, bytes.clone());
                Ok(bytes)
            }
            Err(e) => {
                tracing::error!("Failed to fetch asset {}: {}", resolved_url, e);
                Err(FileError::NotFound(std::path::PathBuf::from(path_str.to_string())))
            }
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        let now = Utc::now();
        Datetime::from_ymd(now.year(), now.month() as u8, now.day() as u8)
    }
}
