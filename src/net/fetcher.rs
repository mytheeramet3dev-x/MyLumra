use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub struct ContentFetcher {
    client: reqwest::Client,
}

impl ContentFetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Lumra/0.2.0 (Typst-Browser; Native-GUI)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self { client }
    }

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

        // 3. Fallback: If not starting with http:// or https://, prepend https://
        let resolved_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            format!("https://{}", trimmed)
        } else {
            trimmed.to_string()
        };

        // 4. Remote HTTP/HTTPS fetch
        let response = self.client
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

    fn handle_internal_page(&self, page: &str) -> Result<String> {
        match page.trim_end_matches('/') {
            "" | "home" | "welcome" => Ok(HOME_PAGE.to_string()),
            "about" => Ok(ABOUT_PAGE.to_string()),
            "cheatsheet" => Ok(CHEATSHEET_PAGE.to_string()),
            other => Err(anyhow!("Unknown internal page: lumra://{}", other)),
        }
    }
}

pub const HOME_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2cm, y: 2.2cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #text(size: 28pt, weight: "bold", fill: rgb("1b3b6f"))[Lumra Browser] \
  #text(size: 13pt, style: "italic", fill: rgb("4a5568"))[Typst-Native Modern Web Browser]
]

#v(1em)

#block(
  fill: rgb("edf2f7"),
  inset: 14pt,
  radius: 8pt,
  stroke: 0.5pt + rgb("cbd5e0"),
)[
  *ยินดีต้อนรับสู่ Lumra Browser!* \
  เบราว์เซอร์ยุคใหม่ที่แสดงผลหน้าเว็บด้วยเอกสาร *Typst (`.typ`)* โดยตรง 
  เพลิดเพลินกับความเร็วระดับมิลลิวินาทีและการจัดหน้าสวยงามระดับสื่อสิ่งพิมพ์ แทนที่ HTML/CSS ดั้งเดิม
]

#v(1.5em)

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
  columns: (1fr, 2fr, 2fr),
  align: (center + horizon, left, left),
  fill: (x, y) => if y == 0 { rgb("e2e8f0") } else { none },
  [*มิติ*], [*HTML / Web เดิม*], [*Lumra + Typst*],
  [การจัดวางหน้า], [ต้องพึ่งพา CSS Box Model ที่ซับซ้อน], [ระบบ Typesetting Engine แท้ระดับโลก],
  [สมการ], [ต้องเรนเดอร์ผ่าน JS (MathJax/KaTeX)], [Native High-Quality Math Engine],
  [ความเร็ว], [รัน Browser Engine ขนาดใหญ่หลายร้อย MB], [Rust Engine ขนาดกะทัดรัด คอมไพล์สดในไม่กี่ ms],
)
"#;

pub const ABOUT_PAGE: &str = r#"
#set page(paper: "a4", margin: (x: 2.5cm, y: 2.5cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
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

== 2. ลิงก์ (Hyperlinks)
- พิมพ์ `#link("https://example.com")[ข้อความที่ต้องการ]`
- หรือพิมพ์ URL ตรงๆ: `https://typst.app`

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
