use std::sync::Arc;
use anyhow::{anyhow, Result};
use egui::ColorImage;
use typst::layout::{Frame, FrameItem, PagedDocument, Point};
use typst::model::Destination;
use super::world::{FontStore, LumraWorld};

#[derive(Clone, Debug)]
pub struct RenderedLink {
    pub rect: egui::Rect,
    pub target: String,
}

pub struct PageRender {
    pub image: ColorImage,
    #[allow(dead_code)]
    pub width_pt: f32,
    #[allow(dead_code)]
    pub height_pt: f32,
    pub links: Vec<RenderedLink>,
}

pub fn compile_and_render(
    font_store: Arc<FontStore>,
    source: &str,
    scale: f32,
) -> Result<Vec<PageRender>> {
    // Ensure math equations always have fallback to New Computer Modern Math if not specified
    let full_source = if !source.contains("math.equation") {
        format!("#show math.equation: set text(font: \"New Computer Modern Math\")\n{}", source)
    } else {
        source.to_string()
    };

    let world = LumraWorld::new(font_store, full_source);
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
        extract_links(&page.frame, Point::zero(), &mut links, scale);

        pages.push(PageRender {
            image: color_image,
            width_pt,
            height_pt,
            links,
        });
    }

    Ok(pages)
}

fn extract_links(frame: &Frame, offset: Point, links: &mut Vec<RenderedLink>, scale: f32) {
    for (pos, item) in frame.items() {
        let current_pos = Point::new(offset.x + pos.x, offset.y + pos.y);
        match item {
            FrameItem::Group(group) => {
                extract_links(&group.frame, current_pos, links, scale);
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
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_typst_document_with_links() {
        let font_store = Arc::new(FontStore::init());
        let source = r#"
            #set text(font: "Libertinus Serif")
            = Test Document with Math
            Hello from Lumra test!
            Here is a math formula:
            $ sum_(k=1)^n k = (n(n+1)) / 2 $
            Here is a link: #link("lumra://home")[Home Link]
        "#;
        let pages = compile_and_render(font_store, source, 1.0).expect("Compilation failed");
        assert_eq!(pages.len(), 1);
        assert!(!pages[0].links.is_empty(), "Link should be detected");
        assert_eq!(pages[0].links[0].target, "lumra://home");
    }
}
