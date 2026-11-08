# ⚡ Flashdrive-Fast

แอปพลิเคชันถ่ายโอนไฟล์ความเร็วสูงพิเศษ (High-Throughput File Transfer) สำหรับ Windows พัฒนาด้วยภาษา **Rust** เน้นความเบา ลื่นไหล และดึงประสิทธิภาพ I/O ออกมาได้สูงสุด

---

## 🚀 จุดเด่นและเทคนิคความเร็วสูง
1. **Pre-allocated Disk Space (`set_len`)**: จองพื้นที่ไฟล์ล่วงหน้าบน Flash Drive / SSD ป้องกันการเกิด Fragmentation (ไฟล์แตกกระจาย) ช่วยลดภาระของ Controller ของแฟลชไดรฟ์
2. **Optimal 4MB Aligned Streaming Buffer**: ใช้บัฟเฟอร์ขนาด 4MB ต่อรอบการอ่านเขียน ซึ่งเป็นขนาดที่ทดสอบแล้วว่าเหมาะกับชิป NAND Flash / USB 3.0 / USB 3.2 และ NVMe มากที่สุด
3. **Hardware Flush & Sync (`sync_all`)**: บังคับ flush ข้อมูลลงไดรฟ์จริงก่อนเสร็จ ป้องกันอาการดึงแฟลชไดรฟ์ออกแล้วไฟล์เสีย
4. **UI เบาและลื่นไหล 60+ FPS (`egui`)**:
   - เรนเดอร์ด้วย Hardware Acceleration (GPU Glow/OpenGL)
   - ไม่ใช้ Webview / Chromium ทำให้ประหยัด RAM (กิน RAM ราวๆ 20-30 MB เท่านั้น)
   - ขนาดตัวโปรแกรม (.exe) เพียง **3.8 MB**
5. **USB Auto-Detect**: ตรวจจับแฟลชไดรฟ์แบบ USB อัตโนมัติ พร้อมแสดงพื้นที่ว่าง และคลิกเลือกเป็นปลายทางได้ในคลิกเดียว
6. **Drag & Drop**: ลากไฟล์หรือโฟลเดอร์จากหน้าจอ Windows Explorer มาวางในแอปได้ทันที

---

## 📦 ตำแหน่งไฟล์โปรแกรมที่พร้อมใช้งาน
ไฟล์ `.exe` ที่คอมไพล์แบบ `--release` (เปิด LTO + Strip):
```text
c:\Users\Woradech\Desktop\flashdrive-fast\target\release\flashdrive-fast.exe
```

---

## 🛠️ วิธีเปิดใช้งาน
- ดับเบิลคลิกที่ไฟล์ `target\release\flashdrive-fast.exe` หรือ
- รันผ่าน Terminal:
  ```powershell
  .\target\release\flashdrive-fast.exe
  ```
