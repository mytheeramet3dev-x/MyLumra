use std::collections::HashMap;
use std::sync::Arc;
use chrono::{Datelike, Utc};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

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
    main_id: FileId,
    source: Source,
    files: HashMap<FileId, Bytes>,
}

impl LumraWorld {
    pub fn new(fonts: Arc<FontStore>, content: String) -> Self {
        let main_id = FileId::new_fake(VirtualPath::new("index.typ"));
        let source = Source::new(main_id, content);

        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            main_id,
            source,
            files: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn set_file(&mut self, path: &str, bytes: Bytes) {
        let id = FileId::new_fake(VirtualPath::new(path));
        self.files.insert(id, bytes);
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
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(std::path::PathBuf::from("Not found")))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(data) = self.files.get(&id) {
            Ok(data.clone())
        } else if id == self.main_id {
            Ok(Bytes::new(self.source.text().as_bytes().to_vec()))
        } else {
            Err(FileError::NotFound(std::path::PathBuf::from("Not found")))
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
