# Lumra Browser

> **A Fast, Modern, Typst-Native Document Web Browser written in Rust**

Lumra เป็นเว็บเบราว์เซอร์ยุคใหม่ที่เปิดอ่านและเรนเดอร์เอกสาร **Typst (`.typ`)** บนอินเทอร์เน็ตโดยตรง แทนที่การใช้ HTML/CSS แบบดั้งเดิม มอบความเร็วในการจัดหน้าในระดับมิลลิวินาที พร้อมคุณภาพการแสดงผลที่สวยงามระดับสื่อสิ่งพิมพ์

---

## ✨ ฟีเจอร์หลัก (Key Features)

- **Native Typst Typesetting Engine:** ใช้ library `typst` และ `typst-render` ในการประมวลผลเอกสาร `.typ` จากซอร์สโค้ดโดยตรงในหน่วยความจำ
- **Crisp High-DPI GUI Viewport:** พัฒนาด้วย `eframe` / `egui` เรนเดอร์เอกสารคมชัดระดับ Retina (2x scaling) พร้อมระบบซูมเข้า-ออก
- **Interactive Hyperlinks (Hit-testing):** สกัดพิกัดกล่องลิงก์จาก Typst layout frames ชี้เมาส์แล้วเคอร์เซอร์เปลี่ยนเป็นรูปมือ และคลิกเพื่อนำทาง (Navigate) ไปยังหน้าถัดไปได้ทันที
- **Multi-Tab & History Management:** รองรับการเปิดหลายแท็บพร้อมกัน มีปุ่ม Back, Forward, Reload และ Address Bar สำหรับพิมพ์ URL
- **Protocol Router:**
  - `https://` และ `http://`: ท่องเว็บดึงไฟล์ `.typ` บนอินเทอร์เน็ต
  - `file://`: เปิดไฟล์ `.typ` ในเครื่องคอมพิวเตอร์
  - `lumra://`: หน้าต่างในตัว เช่น `lumra://home`, `lumra://about`, `lumra://cheatsheet`
- **Zero-Block Async Architecture:** ดึงข้อมูลผ่านเน็ตเวิร์กและคอมไพล์ใน Background thread หน้าต่าง GUI ไม่มีการกระตุกหรือค้าง

---

## 🚀 วิธีการรัน (Run the Browser)

```bash
cd MyLumra
cargo run
```

### การเปิดไฟล์ทดสอบในเครื่อง:
พิมพ์ในช่อง Address bar ด้านบน:
```text
file:///Users/home/Desktop/project/Lumra/sample.typ
```
หรือคลิกลิงก์บนหน้า `lumra://home` ได้ทันที
