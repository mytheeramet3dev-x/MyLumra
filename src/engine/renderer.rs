use std::sync::Arc;
use anyhow::{anyhow, Result};
use egui::ColorImage;
use typst::layout::{Frame, FrameItem, PagedDocument, Point};
use typst::model::Destination;
use super::world::{FontStore, LumraWorld};
use crate::net::ContentFetcher;

const MAX_DOCUMENT_PAGES: usize = 128;
const MAX_RENDER_SCALE: f32 = 4.0;
const MAX_ESTIMATED_PAGE_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_ESTIMATED_DOCUMENT_PIXELS: u64 = 64 * 1024 * 1024;

/// Conservatively estimate output memory before asking the renderer to allocate pixmaps.
/// The factor of two covers point-to-pixel conversion and renderer rounding.
fn validate_render_budget(page_sizes_pt: &[(f64, f64)], scale: f32) -> Result<()> {
    if page_sizes_pt.len() > MAX_DOCUMENT_PAGES {
        return Err(anyhow!("Document exceeds the {} page render limit", MAX_DOCUMENT_PAGES));
    }
    if !scale.is_finite() || scale <= 0.0 || scale > MAX_RENDER_SCALE {
        return Err(anyhow!("Render scale must be greater than zero and no more than {}", MAX_RENDER_SCALE));
    }

    let mut total_pixels = 0_u64;
    for (width_pt, height_pt) in page_sizes_pt {
        if !width_pt.is_finite() || !height_pt.is_finite() || *width_pt <= 0.0 || *height_pt <= 0.0 {
            return Err(anyhow!("Document contains an invalid page size"));
        }
        let width_px = (*width_pt * scale as f64 * 2.0).ceil();
        let height_px = (*height_pt * scale as f64 * 2.0).ceil();
        if !width_px.is_finite() || !height_px.is_finite() || width_px > u32::MAX as f64 || height_px > u32::MAX as f64 {
            return Err(anyhow!("Page dimensions exceed the renderer's safe limits"));
        }
        let pixels = (width_px as u64).saturating_mul(height_px as u64);
        if pixels > MAX_ESTIMATED_PAGE_PIXELS {
            return Err(anyhow!("Page exceeds the {} megapixel render budget", MAX_ESTIMATED_PAGE_PIXELS / 1_000_000));
        }
        total_pixels = total_pixels.saturating_add(pixels);
        if total_pixels > MAX_ESTIMATED_DOCUMENT_PIXELS {
            return Err(anyhow!("Document exceeds the aggregate render pixel budget"));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct RenderedLink {
    pub rect: egui::Rect,
    pub target: String,
}

#[derive(Clone, Debug)]
pub struct RenderedTextSpan {
    pub rect: egui::Rect,
    pub text: String,
}

pub struct PageRender {
    pub image: ColorImage,
    #[allow(dead_code)]
    pub width_pt: f32,
    #[allow(dead_code)]
    pub height_pt: f32,
    pub links: Vec<RenderedLink>,
    pub text_spans: Vec<RenderedTextSpan>,
}

pub fn compile_and_render(
    font_store: Arc<FontStore>,
    fetcher: Arc<ContentFetcher>,
    url: &str,
    source: &str,
    viewport_width: f32,
    viewport_height: f32,
    scale: f32,
    custom_inputs: &std::collections::HashMap<String, String>,
) -> Result<Vec<PageRender>> {
    // Ensure math equations always have fallback to New Computer Modern Math if not specified
    let full_source = if !source.contains("math.equation") {
        format!("#show math.equation: set text(font: \"New Computer Modern Math\")\n{}", source)
    } else {
        source.to_string()
    };

    let world = LumraWorld::new(
        font_store,
        fetcher,
        Some(url.to_string()),
        full_source,
        viewport_width,
        viewport_height,
        custom_inputs,
    );
    let result = typst::compile::<PagedDocument>(&world);

    let document = match result.output {
        Ok(doc) => doc,
        Err(errors) => {
            let mut msg = String::new();
            for err in errors {
                msg.push_str(&format!("Typst error: {:?}\n", err.message));
            }
            return Err(anyhow!(msg));
        }
    };

    let page_sizes: Vec<(f64, f64)> = document.pages.iter()
        .map(|page| (page.frame.size().x.to_pt(), page.frame.size().y.to_pt()))
        .collect();
    validate_render_budget(&page_sizes, scale)?;

    let mut pages = Vec::new();
    for page in &document.pages {
        let pixmap = typst_render::render(page, scale);
        let color_image = ColorImage::from_rgba_premultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );

        let width_pt = page.frame.size().x.to_pt() as f32;
        let height_pt = page.frame.size().y.to_pt() as f32;

        let mut links = Vec::new();
        let mut text_spans = Vec::new();
        extract_items(&page.frame, Point::zero(), &mut links, &mut text_spans, scale);

        pages.push(PageRender {
            image: color_image,
            width_pt,
            height_pt,
            links,
            text_spans,
        });
    }

    Ok(pages)
}

fn extract_items(
    frame: &Frame,
    offset: Point,
    links: &mut Vec<RenderedLink>,
    text_spans: &mut Vec<RenderedTextSpan>,
    scale: f32,
) {
    for (pos, item) in frame.items() {
        let current_pos = Point::new(offset.x + pos.x, offset.y + pos.y);
        match item {
            FrameItem::Group(group) => {
                extract_items(&group.frame, current_pos, links, text_spans, scale);
            }
            FrameItem::Link(dest, size) => {
                let target = match dest {
                    Destination::Url(url) => url.as_str().to_string(),
                    Destination::Position(pos) => format!("#page={}", pos.page.get()),
                    Destination::Location(loc) => format!("#loc={:?}", loc),
                };
                let x = current_pos.x.to_pt() as f32 * scale;
                let y = current_pos.y.to_pt() as f32 * scale;
                let w = size.x.to_pt() as f32 * scale;
                let h = size.y.to_pt() as f32 * scale;
                links.push(RenderedLink {
                    rect: egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h)),
                    target,
                });
            }
            FrameItem::Text(text_item) => {
                let font_size = text_item.size.to_pt() as f32 * scale;
                let x = current_pos.x.to_pt() as f32 * scale;
                let y = (current_pos.y.to_pt() as f32 - text_item.size.to_pt() as f32 * 0.85) * scale;
                let w = text_item.width().to_pt() as f32 * scale;
                let h = font_size;
                text_spans.push(RenderedTextSpan {
                    rect: egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w.max(2.0), h.max(2.0))),
                    text: text_item.text.to_string(),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_budget_rejects_too_many_pages_and_excessive_scale() {
        let too_many = vec![(800.0, 600.0); MAX_DOCUMENT_PAGES + 1];
        assert!(validate_render_budget(&too_many, 1.0).is_err());
        assert!(validate_render_budget(&[(800.0, 600.0)], MAX_RENDER_SCALE + 0.1).is_err());
    }

    #[test]
    fn render_budget_rejects_excessive_page_pixels() {
        assert!(validate_render_budget(&[(20_000.0, 20_000.0)], 2.0).is_err());
        assert!(validate_render_budget(&[(800.0, 600.0)], 1.0).is_ok());
    }

    #[test]
    fn test_compile_typst_document_with_links_and_assets() {
        let font_store = Arc::new(FontStore::init());
        let fetcher = Arc::new(ContentFetcher::new());
        let source = r#"
            #set text(font: "Libertinus Serif")
            = Test Document with Math and Image
            Hello from Lumra test!
            Here is a math formula:
            $ sum_(k=1)^n k = (n(n+1)) / 2 $
            Here is an asset image:
            #image("lumra://assets/logo.svg", width: 40pt)
            Here is a link: #link("lumra://home")[Home Link]
        "#;
        let empty_inputs = std::collections::HashMap::new();
        let pages = compile_and_render(
            font_store,
            fetcher,
            "lumra://test",
            source,
            800.0,
            600.0,
            1.0,
            &empty_inputs,
        )
        .expect("Compilation failed");
        assert_eq!(pages.len(), 1);
        assert!(!pages[0].links.is_empty(), "Link should be detected");
        assert_eq!(pages[0].links[0].target, "lumra://home");
        assert!(!pages[0].text_spans.is_empty(), "Text spans should be extracted");
    }

    #[test]
    fn test_reactive_counter_compilation() {
        let font_store = Arc::new(FontStore::init());
        let fetcher = Arc::new(ContentFetcher::new());
        let source = r#"
            #let count = sys.inputs.at("count", default: "0")
            = Counter Test
            The current count is: #count
            #link("lumra-action://increment?step=1")[Increment]
        "#;

        let mut state = std::collections::HashMap::new();
        state.insert("count".to_string(), "42".to_string());

        let pages = compile_and_render(
            font_store,
            fetcher,
            "lumra://test-counter",
            source,
            800.0,
            600.0,
            1.0,
            &state,
        )
        .expect("Counter compilation failed");

        assert_eq!(pages.len(), 1);
        let contains_42 = pages[0].text_spans.iter().any(|span| span.text.contains("42"));
        assert!(contains_42, "Rendered text should contain state value '42'");
        assert_eq!(pages[0].links[0].target, "lumra-action://increment?step=1");
    }
}
