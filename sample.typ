#set page(paper: "a4", margin: (x: 2cm, y: 2.5cm))
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 11pt)
#show math.equation: set text(font: "New Computer Modern Math")

#align(center)[
  #image("lumra://assets/logo.svg", width: 64pt)
  #v(0.3em)
  #text(size: 24pt, weight: "bold", fill: rgb("1b3b6f"))[Lumra Typst Web Showcase] \
  #text(size: 11pt, fill: rgb("718096"))[ทดสอบระบบ Remote Assets, Responsive Viewport & Continuous Scroll]
]

#v(1em)

== 1. ข้อมูล Viewport & Inputs จากเบราว์เซอร์
เบราว์เซอร์ Lumra ส่งค่าขนาดหน้าจอและ URL ปัจจุบันผ่าน `#sys.inputs`:
- *Viewport Width:* `#sys.inputs.at("viewport-width", default: "800") pt`
- *Viewport Height:* `#sys.inputs.at("viewport-height", default: "600") pt`
- *Current URL:* `#sys.inputs.at("url", default: "sample.typ")`

#v(0.5em)

== 2. การจัดรูปแบบข้อความและสมการ
Typst มีเอนจินการคำนวณสมการคณิตศาสตร์ที่รวดเร็วและสวยงาม:

$ e^(i pi) + 1 = 0 $

$ nabla times bold(B) = mu_0 bold(J) + mu_0 epsilon_0 (diff bold(E)) / (diff t) $

#v(0.5em)

== 3. ไฮเปอร์ลิงก์แบบโต้ตอบ (Interactive Links)
ลองคลิกลิงก์ด้านล่างนี้ได้โดยตรงในเบราว์เซอร์ Lumra:
- 🏠 กลับหน้าหลัก: #link("lumra://home")[ไปยัง lumra://home]
- 📖 คู่มือการใช้งาน: #link("lumra://cheatsheet")[เปิดดู Typst Cheatsheet]
- ℹ️ เกี่ยวกับโปรเจกต์: #link("lumra://about")[เกี่ยวกับ Lumra Browser]

#v(0.5em)

== 4. ตารางสถานะฟีเจอร์ของเบราว์เซอร์
#table(
  columns: (1.3fr, 2.2fr, 1.2fr),
  fill: (x, y) => if y == 0 { rgb("e2e8f0") } else { none },
  [*ฟีเจอร์*], [*รายละเอียด*], [*สถานะ*],
  [Typst Compiler], [เรนเดอร์เอกสาร .typ ตรงๆ ใน Memory], [✅ ใช้งานได้],
  [Remote Assets], [โหลดรูปภาพ (#image) และโมดูล (#import) ผ่านเน็ต], [✅ ใช้งานได้],
  [Continuous Scroll], [สลับมุมมองระหว่าง Web Canvas และ Print View], [✅ ใช้งานได้],
  [Retina Rendering], [ภาพคมชัดสูงด้วย 2x scaling], [✅ ใช้งานได้],
  [Hyperlink Hit-test], [ชี้เมาส์แล้วเคอร์เซอร์เปลี่ยน + กดลิงก์ได้], [✅ ใช้งานได้],
  [Multi-tab & History], [เปิดหลายแท็บและมีปุ่ม Back/Forward], [✅ ใช้งานได้],
)
