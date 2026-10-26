#set page(paper: "a4", margin: 2.5cm)
#set text(font: ("Libertinus Serif", "New Computer Modern Math"), size: 12pt)
#show math.equation: set text(font: "New Computer Modern Math")

= 🌟 เว็บเพจตัวอย่าง Typst (Lumra Web Page)
#v(0.5em)

เอกสารนี้เขียนด้วยภาษา *Typst* และสามารถเปิดอ่านได้โดยตรงบนเบราว์เซอร์ *Lumra* โดยไม่ต้องผ่านการแปลงเป็น HTML หรือ PDF ใดๆ

== 1. การจัดรูปแบบข้อความและสมการ
Typst มีเอนจินการคำนวณสมการคณิตศาสตร์ที่รวดเร็วและสวยงาม:

$ e^(i pi) + 1 = 0 $

$ nabla times bold(B) = mu_0 bold(J) + mu_0 epsilon_0 (diff bold(E)) / (diff t) $

== 2. ไฮเปอร์ลิงก์แบบโต้ตอบ (Interactive Links)
ลองคลิกลิงก์ด้านล่างนี้ได้โดยตรงในเบราว์เซอร์ Lumra:
- 🏠 กลับหน้าหลัก: #link("lumra://home")[ไปยัง lumra://home]
- 📖 คู่มือการใช้งาน: #link("lumra://cheatsheet")[เปิดดู Typst Cheatsheet]
- ℹ️ เกี่ยวกับโปรเจกต์: #link("lumra://about")[เกี่ยวกับ Lumra Browser]

== 3. ตารางข้อมูล (Native Table Layout)
#table(
  columns: (1fr, 2fr, 1.5fr),
  fill: (x, y) => if y == 0 { rgb("e2e8f0") } else { none },
  [*ฟีเจอร์*], [*รายละเอียด*], [*สถานะ*],
  [Typst Compiler], [เรนเดอร์เอกสาร .typ ตรงๆ ใน Memory], [✅ ใช้งานได้],
  [Retina Rendering], [ภาพคมชัดสูงด้วย 2x scaling], [✅ ใช้งานได้],
  [Hyperlink Hit-test], [ชี้เมาส์แล้วเคอร์เซอร์เปลี่ยน + กดลิงก์ได้], [✅ ใช้งานได้],
  [Multi-tab & History], [เปิดหลายแท็บและมีปุ่ม Back/Forward], [✅ ใช้งานได้],
)
