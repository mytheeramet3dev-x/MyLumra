use std::collections::HashMap;

use egui::Rect;

use super::renderer::PageRender;

/// Document-level information derived from a compiled Typst document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub url: String,
    pub title: String,
    pub source_bytes: usize,
    pub page_count: usize,
}

/// A searchable text fragment and its location in the rendered document.
#[derive(Debug, Clone)]
pub struct TextEntry {
    pub page_index: usize,
    pub rect: Rect,
    pub text: String,
}

/// A navigable link and its location in the rendered document.
#[derive(Debug, Clone)]
pub struct LinkEntry {
    pub page_index: usize,
    pub rect: Rect,
    pub target: String,
}

/// A lightweight index over a compiled document.
///
/// Pixel-space rectangles use the same coordinate system as PageRender, so the
/// viewport can consume these entries without translating Typst layout units.
#[derive(Debug, Clone)]
pub struct DocumentModel {
    pub metadata: DocumentMetadata,
    pub text_index: Vec<TextEntry>,
    pub links: Vec<LinkEntry>,
    /// Fragment targets found in links, grouped by target (for in-document navigation).
    pub anchors: HashMap<String, Vec<(usize, Rect)>>,
}

impl DocumentModel {
    pub fn from_rendered(url: &str, source: &str, pages: &[PageRender]) -> Self {
        let mut text_index = Vec::new();
        let mut links = Vec::new();
        let mut anchors: HashMap<String, Vec<(usize, Rect)>> = HashMap::new();

        for (page_index, page) in pages.iter().enumerate() {
            text_index.extend(page.text_spans.iter().map(|span| TextEntry {
                page_index,
                rect: span.rect,
                text: span.text.clone(),
            }));

            for link in &page.links {
                if link.target.starts_with('#') {
                    anchors
                        .entry(link.target.clone())
                        .or_default()
                        .push((page_index, link.rect));
                }
                links.push(LinkEntry {
                    page_index,
                    rect: link.rect,
                    target: link.target.clone(),
                });
            }
        }

        Self {
            metadata: DocumentMetadata {
                url: url.to_string(),
                title: extract_title(source, url),
                source_bytes: source.len(),
                page_count: pages.len(),
            },
            text_index,
            links,
            anchors,
        }
    }

    /// Return matching text entries using a case-insensitive substring search.
    pub fn search_text<'a>(&'a self, query: &str) -> Vec<&'a TextEntry> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let query = query.to_lowercase();
        self.text_index
            .iter()
            .filter(|entry| entry.text.to_lowercase().contains(&query))
            .collect()
    }

    pub fn links_for_target<'a>(&'a self, target: &str) -> Vec<&'a LinkEntry> {
        self.links
            .iter()
            .filter(|link| link.target == target)
            .collect()
    }
}

fn extract_title(source: &str, url: &str) -> String {
    source
        .lines()
        .map(str::trim)
        .find_map(|line| {
            let heading = line.strip_prefix('=')?;
            let title = heading.trim();
            if title.is_empty() {
                None
            } else {
                Some(title.trim_start_matches('=').trim().to_string())
            }
        })
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| {
            url.rsplit('/')
                .find(|part| !part.is_empty())
                .unwrap_or(url)
                .to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::renderer::{RenderedLink, RenderedTextSpan};
    use egui::{pos2, vec2};

    fn test_page() -> PageRender {
        PageRender {
            image: egui::ColorImage::new([1, 1], egui::Color32::WHITE),
            width_pt: 100.0,
            height_pt: 100.0,
            links: vec![RenderedLink {
                rect: Rect::from_min_size(pos2(1.0, 2.0), vec2(10.0, 4.0)),
                target: "#intro".to_string(),
            }],
            text_spans: vec![RenderedTextSpan {
                rect: Rect::from_min_size(pos2(3.0, 4.0), vec2(20.0, 8.0)),
                text: "Hello Lumra".to_string(),
            }],
        }
    }

    #[test]
    fn builds_metadata_and_indexes_from_rendered_pages() {
        let pages = vec![test_page()];
        let model = DocumentModel::from_rendered(
            "lumra://docs/intro",
            "= Introduction\nHello Lumra",
            &pages,
        );

        assert_eq!(model.metadata.title, "Introduction");
        assert_eq!(model.metadata.page_count, 1);
        assert_eq!(model.text_index.len(), 1);
        assert_eq!(model.links.len(), 1);
        assert_eq!(model.anchors.get("#intro").map(Vec::len), Some(1));
    }

    #[test]
    fn searches_text_case_insensitively() {
        let pages = vec![test_page()];
        let model = DocumentModel::from_rendered("lumra://docs/intro", "", &pages);

        assert_eq!(model.search_text("lumra").len(), 1);
        assert!(model.search_text("missing").is_empty());
        assert!(model.search_text("  ").is_empty());
    }

    #[test]
    fn falls_back_to_url_when_no_heading_exists() {
        let model = DocumentModel::from_rendered(
            "https://example.test/guide",
            "Plain text without a heading",
            &[],
        );

        assert_eq!(model.metadata.title, "guide");
    }
}
