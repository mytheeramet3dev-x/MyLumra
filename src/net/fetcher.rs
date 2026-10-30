use anyhow::{anyhow, Result};
use bytes::Bytes;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use url::Url;

pub struct ContentFetcher {
    async_client: reqwest::Client,
    blocking_client: reqwest::blocking::Client,
    asset_cache: Mutex<HashMap<String, Bytes>>,
}

impl ContentFetcher {
    pub fn new() -> Self {
        let async_client = reqwest::Client::builder()
            .user_agent("Lumra/0.2.0 (Typst-Browser; Native-GUI)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        let blocking_client = reqwest::blocking::Client::builder()
            .user_agent("Lumra/0.2.0 (Typst-Browser; Native-GUI)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        Self {
            async_client,
            blocking_client,
            asset_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve an asset path against an optional base URL/path
    pub fn resolve_url(&self, base_url: Option<&str>, path_str: &str) -> String {
        let trimmed = path_str.trim();

        // 1. Fix Unix VirtualPath single-slash artifact for protocols
        let normalized = Self::normalize_url(trimmed);

        // 2. Already an absolute URL
        if normalized.starts_with("http://")
            || normalized.starts_with("https://")
            || normalized.starts_with("file://")
            || normalized.starts_with("lumra://")
        {
            return normalized;
        }

        // 3. Resolve relative to base URL
        if let Some(base) = base_url {
            let base_norm = Self::normalize_url(base.trim());
            if let Ok(base_parsed) = Url::parse(&base_norm) {
                if let Ok(joined) = base_parsed.join(&normalized) {
                    return joined.to_string();
                }
            } else if base_norm.starts_with("lumra://") {
                return format!("{}/{}", base_norm.trim_end_matches('/'), normalized.trim_start_matches('/'));
            } else {
                // Base is a local file path
                let base_path = Path::new(&base_norm);
                let dir = if base_path.is_file() {
                    base_path.parent().unwrap_or(Path::new("."))
                } else {
                    base_path
                };
                let joined = dir.join(&normalized);
                return joined.to_string_lossy().to_string();
            }
        }

        normalized
    }

    pub fn normalize_url(raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.starts_with("https:/") && !trimmed.starts_with("https://") {
            format!("https://{}", &trimmed["https:/".len()..])
        } else if trimmed.starts_with("http:/") && !trimmed.starts_with("http://") {
            format!("http://{}", &trimmed["http:/".len()..])
        } else if trimmed.starts_with("lumra:/") && !trimmed.starts_with("lumra://") {
            format!("lumra://{}", &trimmed["lumra:/".len()..])
        } else if trimmed.starts_with("file:/") && !trimmed.starts_with("file://") {
            format!("file://{}", &trimmed["file:/".len()..])
        } else {
            trimmed.to_string()
        }
    }

    /// Asynchronous fetch for top-level pages
    pub async fn fetch(&self, url_str: &str) -> Result<String> {
        let trimmed = url_str.trim();

        // 1. Internal lumra:// scheme
        if let Some(path) = trimmed.strip_prefix("lumra://") {
            return self.handle_internal_page(path);
        }

        // 2. Local file:// scheme
        if let Some(path) = trimmed.strip_prefix("file://") {
            let path_buf = PathBuf::from(path);
            return std::fs::read_to_string(&path_buf)
                .map_err(|e| anyhow!("Failed to read file {}: {}", path, e));
        }

        // Check if direct local path
        if Path::new(trimmed).is_file() {
            return std::fs::read_to_string(trimmed)
                .map_err(|e| anyhow!("Failed to read file {}: {}", trimmed, e));
        }

        // 3. Remote HTTP/HTTPS fetch
        let resolved_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        let response = self
            .async_client
            .get(&resolved_url)
            .header("Accept", "text/typst, text/plain;q=0.9, */*;q=0.8")
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP Status: {}", response.status()));
        }

        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to decode response: {}", e))?;

        Ok(text)
    }

    /// Synchronous fetch for text (used by #import and #include)
    pub fn fetch_text_blocking(&self, url_str: &str) -> Result<String> {
        let bytes = self.fetch_bytes_blocking(url_str)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| anyhow!("UTF-8 decode error for {}: {}", url_str, e))
    }

    /// Synchronous fetch for raw bytes (used by #image, #read, etc.)
    pub fn fetch_bytes_blocking(&self, url_str: &str) -> Result<Bytes> {
        let normalized = Self::normalize_url(url_str);

        // 1. Check in-memory asset cache
        {
            let cache = self.asset_cache.lock().unwrap();
            if let Some(cached) = cache.get(&normalized) {
                return Ok(cached.clone());
            }
        }

        let trimmed = normalized.trim();

        // 2. Handle internal lumra:// assets
        if let Some(path) = trimmed.strip_prefix("lumra://") {
            if let Some(bytes) = self.handle_internal_asset(path) {
                self.cache_asset(&normalized, bytes.clone());
                return Ok(bytes);
            }
            return Err(anyhow!("Unknown lumra asset: {}", trimmed));
        }

        // 3. Handle file:// scheme
        if let Some(path) = trimmed.strip_prefix("file://") {
            let data = std::fs::read(path)
                .map_err(|e| anyhow!("Failed to read local asset {}: {}", path, e))?;
            let bytes = Bytes::from(data);
            self.cache_asset(&normalized, bytes.clone());
            return Ok(bytes);
        }

        // 4. Handle direct local filesystem path
        let local_path = Path::new(trimmed);
        if local_path.is_file() {
            let data = std::fs::read(local_path)
                .map_err(|e| anyhow!("Failed to read local file {}: {}", trimmed, e))?;
            let bytes = Bytes::from(data);
            self.cache_asset(url_str, bytes.clone());
            return Ok(bytes);
        }

        // 5. Remote HTTP/HTTPS fetch
        let resolved_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        let response = self
            .blocking_client
            .get(&resolved_url)
            .header("Accept", "image/*, application/octet-stream, text/*, */*")
            .send()
            .map_err(|e| anyhow!("Failed to request {}: {}", resolved_url, e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP Status {} for {}", response.status(), resolved_url));
        }

        let raw = response
            .bytes()
            .map_err(|e| anyhow!("Failed to download {}: {}", resolved_url, e))?;

        self.cache_asset(url_str, raw.clone());
        Ok(raw)
    }

    fn cache_asset(&self, key: &str, bytes: Bytes) {
        let mut cache = self.asset_cache.lock().unwrap();
        cache.insert(key.to_string(), bytes);
    }

    fn handle_internal_asset(&self, asset_path: &str) -> Option<Bytes> {
        match asset_path.trim_start_matches('/') {
            "logo.svg" | "assets/logo.svg" => Some(Bytes::from_static(LUMRA_LOGO_SVG.as_bytes())),
            _ => None,
        }
    }

    fn handle_internal_page(&self, page: &str) -> Result<String> {
        match page.trim_end_matches('/') {
            "" | "home" | "welcome" => Ok(HOME_PAGE.to_string()),
            "about" => Ok(ABOUT_PAGE.to_string()),
            "cheatsheet" => Ok(CHEATSHEET_PAGE.to_string()),
            other => Err(anyhow!("Unknown internal page: lumra://{}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_resolution() {
        let fetcher = ContentFetcher::new();

        // 1. Relative image on remote site
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/article.typ"), "banner.png");
        assert_eq!(resolved, "https://example.com/blog/banner.png");

        // 2. Parent directory relative image
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/article.typ"), "../images/logo.svg");
        assert_eq!(resolved, "https://example.com/images/logo.svg");

        // 3. Absolute URL should remain absolute
        let resolved = fetcher.resolve_url(Some("https://example.com/blog/"), "https://cdn.foo.com/pic.jpg");
        assert_eq!(resolved, "https://cdn.foo.com/pic.jpg");

        // 4. VirtualPath single slash normalization
        let resolved = fetcher.resolve_url(None, "lumra:/assets/logo.svg");
        assert_eq!(resolved, "lumra://assets/logo.svg");

        // 5. File URL resolution
        let resolved = fetcher.resolve_url(Some("file:///Users/test/index.typ"), "components.typ");
        assert_eq!(resolved, "file:///Users/test/components.typ");
    }
}

pub const LUMRA_LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <defs>
    <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#3b82f6" />
      <stop offset="100%" stop-color="#1d4ed8" />
    </linearGradient>
  </defs>
  <rect width="100" height="100" rx="22" fill="url(#grad)" />
  <circle cx="50" cy="50" r="32" fill="none" stroke="#ffffff" stroke-width="6" opacity="0.4" />
  <path d="M34 28 L34 72 L68 72" fill="none" stroke="#ffffff" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="68" cy="40" r="7" fill="#60a5fa" />
</svg>"##;

pub const HOME_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2cm, y: 2.2cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #image("lumra://assets/logo.svg", width: 56pt)
  #v(0.3em)
  #text(size: 26pt, weight: "bold", fill: rgb("1b3b6f"))[Lumra Browser] \
  #text(size: 12pt, style: "italic", fill: rgb("4a5568"))[The Fast, Typeset-Native Web Browser]
]

#v(0.8em)

#block(
  fill: rgb("edf2f7"),
  inset: 14pt,
  radius: 8pt,
  stroke: 0.5pt + rgb("cbd5e0"),
)[
  *ยินดีต้อนรับสู่ Lumra Browser!* \
  เบราว์เซอร์ยุคใหม่ที่แสดงผลหน้าเว็บด้วยเอกสาร *Typst (`.typ`)* โดยตรง 
  เพลิดเพลินกับความเร็วระดับมิลลิวินาทีและการจัดหน้าสวยงามระดับสื่อสิ่งพิมพ์ แทนที่ HTML/CSS ดั้งเดิม
  รองรับรูปภาพ Remote Assets (#raw("lumra://assets/logo.svg")), ฟอนต์, สูตรคณิตศาสตร์ และไฮเปอร์ลิงก์
]

#v(1em)

== 🧭 เริ่มต้นการสำรวจ
- เข้าชมเอกสารแนะนำ: #link("lumra://cheatsheet")[คู่มือ Typst Cheatsheet ฉบับรวดเร็ว]
- เกี่ยวกับเบราว์เซอร์: #link("lumra://about")[ข้อมูลและสถาปัตยกรรมของ Lumra]
- หรือพิมพ์ URL เว็บไซต์ หรือ `file:///path/to/file.typ` ในแถบด้านบนเพื่อเปิดดูเอกสาร

#v(1em)

== 📐 พลังของ Typst Typesetting
รองรับการเขียนสมการคณิตศาสตร์และตารางสไตล์สื่อสิ่งพิมพ์อย่างเป็นธรรมชาติ:

$ sum_(k=1)^n k = (n(n+1)) / 2 $

$ f(x) = integral_(-infinity)^x 1 / (sigma sqrt(2 pi)) e^(- (t - mu)^2 / (2 sigma^2)) dif t $

#v(1em)

#table(
  columns: (1.2fr, 2fr, 2fr),
  align: (center + horizon, left, left),
  fill: (x, y) => if y == 0 { rgb("e2e8f0") } else { none },
  [*มิติ*], [*HTML / Web เดิม*], [*Lumra + Typst*],
  [การจัดวางหน้า], [ต้องพึ่งพา CSS Box Model ที่ซับซ้อน], [ระบบ Typesetting Engine แท้ระดับโลก],
  [รูปภาพและ Assets], [ต้องเขียนแท็ก <img> พร้อม JS polyfills], [Native Remote Asset Fetcher ในตัว],
  [สมการคณิตศาสตร์], [ต้องเรนเดอร์ผ่าน JS (MathJax/KaTeX)], [Native High-Quality Math Engine],
  [ความเร็ว], [รัน Browser Engine ขนาดใหญ่หลายร้อย MB], [Rust Engine ขนาดกะทัดรัด คอมไพล์สดในไม่กี่ ms],
)
"#;

pub const ABOUT_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2.5cm, y: 2.5cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #image("lumra://assets/logo.svg", width: 48pt)
  #v(0.3em)
  #text(size: 24pt, weight: "bold", fill: rgb("1b3b6f"))[เกี่ยวกับ Lumra Browser] \
  #text(size: 11pt, fill: rgb("718096"))[เวอร์ชัน 0.2.0 (Typst Engine Edition)]
]

#v(1.5em)

== 🎯 วิสัยทัศน์
*Lumra* มีเป้าหมายในการสร้างมาตรฐานใหม่สำหรับการอ่านและนำเสนอเนื้อหาบนอินเทอร์เน็ต โดยใช้ *Typst* เป็นภาษาหลักในการเขียนเว็บแทนที่ HTML/CSS 

ทำให้เว็บเพจ:
1. *เร็วและเบาที่สุด* — ไม่มี JavaScript bloatware หรือ CSS framework ที่หนักเครื่อง
2. *สวยงาม ไร้ที่ติ* — ทุกตัวอักษร ช่องไฟ สมการ และรูปภาพได้รับการจัดวางด้วยมาตรฐานสิ่งพิมพ์ระดับมืออาชีพ
3. *ปลอดภัย 100%* — ไม่มีการรันสคริปต์สอดแนม หรือมัลแวร์ในระดับเบราว์เซอร์
4. *เชื่อมต่อสินทรัพย์ผ่านเว็บ* — รองรับการโหลดรูปภาพ โมดูล และฟอนต์ผ่านเน็ตเวิร์กได้อย่างราบรื่น

#v(1em)
กลับสู่: #link("lumra://home")[หน้าแรก Lumra Home]
"#;

pub const CHEATSHEET_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2cm, y: 2cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

= 📝 Typst Syntax Cheatsheet

== 1. หัวข้อและข้อความ
- `= Heading 1`, `== Heading 2`, `=== Heading 3`
- `*ตัวหนา (Bold)*` และ `_ตัวเอียง (Italic)_`
- ขีดฆ่า: `~Strikethrough~`

== 2. ลิงก์และรูปภาพ (Hyperlinks & Images)
- ลิงก์: `#link("https://example.com")[ข้อความที่ต้องการ]`
- รูปภาพ: `#image("lumra://assets/logo.svg", width: 40pt)`
- รูปภาพจากเน็ต: `#image("https://example.com/photo.png", width: 100%)`

== 3. สมการคณิตศาสตร์ (Math)
- สมการในบรรทัด: `$x + y = z$`
- สมการแยกบรรทัด:
$ A = mat(
  1, 2;
  3, 4
) $

== 4. บล็อกโค้ด (Code Blocks)
```rust
fn main() {
    println!("Hello from Lumra Browser!");
}
```

#v(1em)
กลับสู่: #link("lumra://home")[หน้าแรก Lumra Home]
"#;
