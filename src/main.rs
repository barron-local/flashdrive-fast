#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drives;
mod engine;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, Frame, Layout, Margin,
    ProgressBar, RichText, Rounding, Stroke, Vec2,
};

use drives::{get_available_drives, DriveInfo};
use engine::{FastCopier, TransferProgress, TransferStats};

// Notion Colors
const NOTION_BG: Color32 = Color32::from_rgb(25, 25, 25);
const NOTION_CARD: Color32 = Color32::from_rgb(32, 32, 32);
const NOTION_BORDER: Color32 = Color32::from_rgb(46, 46, 46);
const NOTION_TEXT_MAIN: Color32 = Color32::from_rgb(235, 235, 235);
const NOTION_TEXT_MUTED: Color32 = Color32::from_rgb(140, 140, 140);
const NOTION_BLUE: Color32 = Color32::from_rgb(35, 131, 226);

#[derive(PartialEq, Clone, Copy)]
enum AppScreen {
    SelectDrive,
    ManageDrive,
}

#[derive(PartialEq, Clone, Copy)]
enum TransferDirection {
    PCtoDrive,
    DriveToPC,
}

#[derive(PartialEq, Clone, Copy)]
enum OperationMode {
    Copy,
    Move,
}

struct FastTransferApp {
    screen: AppScreen,
    drives: Vec<DriveInfo>,
    selected_drive: Option<DriveInfo>,

    direction: TransferDirection,
    mode: Option<OperationMode>,
    selected_items: Vec<PathBuf>,
    pc_folder: Option<PathBuf>,

    toast_message: Option<(String, std::time::Instant)>,

    is_running: bool,
    stats: Option<Arc<TransferStats>>,
    progress: Option<TransferProgress>,
    rx_progress: Option<Receiver<TransferProgress>>,
    status_text: String,
    last_refresh: std::time::Instant,
}

impl Default for FastTransferApp {
    fn default() -> Self {
        let drives = get_available_drives();
        Self {
            screen: AppScreen::SelectDrive,
            drives,
            selected_drive: None,
            direction: TransferDirection::PCtoDrive,
            mode: None,
            selected_items: Vec::new(),
            pc_folder: None,
            toast_message: None,
            is_running: false,
            stats: None,
            progress: None,
            rx_progress: None,
            status_text: "พร้อมทำงาน".to_string(),
            last_refresh: std::time::Instant::now(),
        }
    }
}

impl FastTransferApp {
    fn setup_custom_fonts(ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();

        let font_candidates = [
            "C:\\Windows\\Fonts\\leelawdb.ttf",
            "C:\\Windows\\Fonts\\LeelaUIb.ttf",
            "C:\\Windows\\Fonts\\tahoma.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
        ];

        let mut thai_font_data = None;
        for path in font_candidates {
            if let Ok(data) = fs::read(path) {
                thai_font_data = Some(data);
                break;
            }
        }

        if let Some(font_bytes) = thai_font_data {
            fonts.font_data.insert(
                "thai_notion".to_owned(),
                FontData::from_owned(font_bytes),
            );

            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "thai_notion".to_owned());

            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "thai_notion".to_owned());

            ctx.set_fonts(fonts);
        }
    }

    fn show_toast(&mut self, msg: &str) {
        self.toast_message = Some((msg.to_string(), std::time::Instant::now()));
    }

    fn refresh_drives(&mut self) {
        self.drives = get_available_drives();
        self.last_refresh = std::time::Instant::now();
    }

    fn start_transfer(&mut self) {
        if self.mode.is_none() {
            self.show_toast("กรุณาเลือกรูปแบบการทำงานก่อน (คัดลอก หรือ ย้าย)");
            return;
        }

        if self.selected_items.is_empty() {
            self.show_toast("กรุณาเลือกไฟล์หรือโฟลเดอร์ที่จะส่งก่อน");
            return;
        }

        let drive = match &self.selected_drive {
            Some(d) => d,
            None => {
                self.show_toast("กรุณาเลือกไดรฟ์ก่อน");
                return;
            }
        };

        let destination = match self.direction {
            TransferDirection::PCtoDrive => PathBuf::from(&drive.mount_point),
            TransferDirection::DriveToPC => match &self.pc_folder {
                Some(f) => f.clone(),
                None => {
                    self.show_toast("กรุณาเลือกโฟลเดอร์บนคอมพิวเตอร์ที่ต้องการบันทึก");
                    return;
                }
            },
        };

        let stats = Arc::new(TransferStats::new());
        self.stats = Some(Arc::clone(&stats));

        let (tx, rx): (Sender<TransferProgress>, Receiver<TransferProgress>) = unbounded();
        self.rx_progress = Some(rx);
        self.is_running = true;
        self.progress = None;

        let sources = self.selected_items.clone();
        let is_move = self.mode == Some(OperationMode::Move);
        let buf_size = 8 * 1024 * 1024;

        thread::spawn(move || {
            let copier = FastCopier {
                buffer_size: buf_size,
                is_move,
            };

            copier.execute(sources, destination, stats, move |prog| {
                let _ = tx.send(prog);
            });
        });
    }

    fn cancel_transfer(&mut self) {
        if let Some(stats) = &self.stats {
            stats.cancel_flag.store(true, Ordering::Relaxed);
            self.status_text = "กำลังยกเลิก...".to_string();
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

impl eframe::App for FastTransferApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_refresh.elapsed() > Duration::from_secs(4) {
            self.refresh_drives();
        }

        if let Some(rx) = &self.rx_progress {
            while let Ok(prog) = rx.try_recv() {
                if prog.is_finished {
                    self.is_running = false;
                    if let Some(err) = &prog.error {
                        self.status_text = format!("ข้อผิดพลาด: {}", err);
                    } else {
                        self.status_text = String::new(); // Clear text to avoid redundancy
                    }
                }
                self.progress = Some(prog);
            }
        }

        // Drag & Drop
        if self.screen == AppScreen::ManageDrive {
            ctx.input(|i| {
                if !i.raw.dropped_files.is_empty() {
                    for file in &i.raw.dropped_files {
                        if let Some(path) = &file.path {
                            if !self.selected_items.contains(path) {
                                self.selected_items.push(path.clone());
                            }
                        }
                    }
                }
            });
        }

        if self.is_running || self.toast_message.is_some() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = NOTION_BG;
        visuals.window_fill = NOTION_BG;
        visuals.extreme_bg_color = NOTION_CARD;
        visuals.widgets.noninteractive.bg_fill = NOTION_CARD;
        visuals.widgets.inactive.bg_fill = NOTION_CARD;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 42, 42);
        visuals.widgets.active.bg_fill = Color32::from_rgb(48, 48, 48);
        visuals.selection.bg_fill = NOTION_BLUE;
        ctx.set_visuals(visuals);

        egui::CentralPanel::default()
            .frame(Frame::none().fill(NOTION_BG).inner_margin(Margin::same(24.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    match self.screen {
                        AppScreen::SelectDrive => self.render_select_drive_screen(ui),
                        AppScreen::ManageDrive => self.render_manage_drive_screen(ui),
                    }

                    // Render Toast Notification
                    if let Some((msg, created_at)) = &self.toast_message.clone() {
                        let elapsed = created_at.elapsed();
                        if elapsed > Duration::from_secs(3) {
                            self.toast_message = None;
                        } else {
                            ui.add_space(8.0);
                            Frame::none()
                                .fill(Color32::from_rgb(45, 30, 20))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(220, 120, 40)))
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(14.0, 8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("[เตือน] {}", msg))
                                                .size(13.0)
                                                .strong()
                                                .color(Color32::from_rgb(255, 180, 80)),
                                        );
                                    });
                                });
                        }
                    }
                });
            });
    }
}

impl FastTransferApp {
    // SCREEN 1: SELECT DRIVE
    fn render_select_drive_screen(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = Vec2::new(16.0, 16.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("เลือกไดรฟ์ที่ต้องการใช้งาน")
                    .size(24.0)
                    .strong()
                    .color(NOTION_TEXT_MAIN),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("รีเฟรช").clicked() {
                    self.refresh_drives();
                }
            });
        });

        ui.label(
            RichText::new("เชื่อมต่อแฟลชไดรฟ์หรือเลือกไดรฟ์ในเครื่องเพื่อเริ่มจัดการไฟล์")
                .size(13.0)
                .color(NOTION_TEXT_MUTED),
        );

        ui.separator();

        for drive in &self.drives {
            let is_usb = drive.is_removable;
            let border_color = if is_usb {
                Color32::from_rgb(35, 131, 226)
            } else {
                NOTION_BORDER
            };

            let card = Frame::none()
                .fill(NOTION_CARD)
                .stroke(Stroke::new(1.0_f32, border_color))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(16.0));

            card.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let type_str = if is_usb { "[USB Flash Drive]" } else { "[Local Disk]" };
                        ui.label(
                            RichText::new(format!("{}  {}", drive.mount_point, drive.name))
                                .size(16.0)
                                .strong()
                                .color(NOTION_TEXT_MAIN),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!(
                                "{}  •  พื้นที่ว่าง {} จากทั้งหมด {}",
                                type_str,
                                format_bytes(drive.available_space),
                                format_bytes(drive.total_space)
                            ))
                            .size(12.0)
                            .color(NOTION_TEXT_MUTED),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let btn = ui.add_sized(
                            [120.0, 34.0],
                            egui::Button::new(
                                RichText::new("เข้าใช้งาน")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(NOTION_BLUE),
                        );

                        if btn.clicked() {
                            self.selected_drive = Some(drive.clone());
                            self.selected_items.clear();
                            self.mode = None;
                            self.toast_message = None;
                            self.status_text = "พร้อมทำงาน".to_string();
                            self.screen = AppScreen::ManageDrive;
                        }
                    });
                });
            });
        }
    }

    // SCREEN 2: MANAGE DRIVE
    fn render_manage_drive_screen(&mut self, ui: &mut egui::Ui) {
        let drive = match &self.selected_drive {
            Some(d) => d.clone(),
            None => {
                self.screen = AppScreen::SelectDrive;
                return;
            }
        };

        ui.spacing_mut().item_spacing = Vec2::new(14.0, 14.0);

        // Header
        ui.horizontal(|ui| {
            if ui.button("< เปลี่ยนไดรฟ์").clicked() {
                if !self.is_running {
                    self.screen = AppScreen::SelectDrive;
                }
            }

            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("ไดรฟ์: {} ({})", drive.mount_point, drive.name))
                    .size(20.0)
                    .strong()
                    .color(NOTION_TEXT_MAIN),
            );

            ui.label(
                RichText::new(format!("(ว่าง {})", format_bytes(drive.available_space)))
                    .size(13.0)
                    .color(NOTION_TEXT_MUTED),
            );
        });

        ui.separator();

        // 1. DIRECTION TABS
        ui.horizontal(|ui| {
            let to_drive_active = self.direction == TransferDirection::PCtoDrive;
            let to_pc_active = self.direction == TransferDirection::DriveToPC;

            let btn1 = ui.add_sized(
                [190.0, 34.0],
                egui::Button::new(
                    RichText::new("นำเข้าไฟล์ -> แฟลชไดรฟ์")
                        .size(13.0)
                        .strong()
                        .color(if to_drive_active { Color32::WHITE } else { NOTION_TEXT_MUTED }),
                )
                .fill(if to_drive_active { NOTION_BLUE } else { NOTION_CARD }),
            );
            if btn1.clicked() {
                self.direction = TransferDirection::PCtoDrive;
                self.selected_items.clear();
            }

            let btn2 = ui.add_sized(
                [190.0, 34.0],
                egui::Button::new(
                    RichText::new("ดึงไฟล์จากไดรฟ์ -> ลงคอม")
                        .size(13.0)
                        .strong()
                        .color(if to_pc_active { Color32::WHITE } else { NOTION_TEXT_MUTED }),
                )
                .fill(if to_pc_active { NOTION_BLUE } else { NOTION_CARD }),
            );
            if btn2.clicked() {
                self.direction = TransferDirection::DriveToPC;
                self.selected_items.clear();
            }
        });

        // 2. FILE SELECTION BOX
        Frame::none()
            .fill(NOTION_CARD)
            .stroke(Stroke::new(1.0_f32, NOTION_BORDER))
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::same(16.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = match self.direction {
                        TransferDirection::PCtoDrive => "เลือกไฟล์จากคอมพิวเตอร์ที่ต้องการส่งเข้าไดรฟ์",
                        TransferDirection::DriveToPC => "เลือกไฟล์ในแฟลชไดรฟ์ที่ต้องการดึงเข้าคอมพิวเตอร์",
                    };
                    ui.label(RichText::new(title).size(13.0).strong().color(NOTION_TEXT_MAIN));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !self.selected_items.is_empty() && ui.button("ล้างรายการ").clicked() {
                            self.selected_items.clear();
                        }
                        if ui.button("+ โฟลเดอร์").clicked() {
                            let mut dialog = rfd::FileDialog::new();
                            if self.direction == TransferDirection::DriveToPC {
                                dialog = dialog.set_directory(&drive.mount_point);
                            }
                            if let Some(folder) = dialog.pick_folder() {
                                if !self.selected_items.contains(&folder) {
                                    self.selected_items.push(folder);
                                }
                            }
                        }
                        if ui.button("+ ไฟล์").clicked() {
                            let mut dialog = rfd::FileDialog::new();
                            if self.direction == TransferDirection::DriveToPC {
                                dialog = dialog.set_directory(&drive.mount_point);
                            }
                            if let Some(files) = dialog.pick_files() {
                                for f in files {
                                    if !self.selected_items.contains(&f) {
                                        self.selected_items.push(f);
                                    }
                                }
                            }
                        }
                    });
                });

                ui.separator();

                if self.selected_items.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(25.0);
                        ui.label(RichText::new("ลากไฟล์หรือโฟลเดอร์มาวางในกล่องนี้ได้เลย").size(13.0).color(NOTION_TEXT_MUTED));
                        ui.label(RichText::new("หรือกดปุ่ม + ไฟล์ / + โฟลเดอร์ ด้านบน").size(11.0).color(NOTION_TEXT_MUTED));
                        ui.add_space(15.0);
                    });
                } else {
                    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                        let mut to_remove = None;
                        for (idx, p) in self.selected_items.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.small_button("X").clicked() {
                                    to_remove = Some(idx);
                                }
                                let tag = if p.is_dir() { "[DIR]" } else { "[FILE]" };
                                let name = p.file_name().map_or("Unknown", |n| n.to_str().unwrap_or(""));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!("{} {}", tag, name))
                                            .size(12.0)
                                            .color(NOTION_TEXT_MAIN),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                        if let Some(idx) = to_remove {
                            self.selected_items.remove(idx);
                        }
                    });
                }
            });

        // 3. TARGET DESTINATION (DriveToPC only)
        if self.direction == TransferDirection::DriveToPC {
            Frame::none()
                .fill(NOTION_CARD)
                .stroke(Stroke::new(1.0_f32, NOTION_BORDER))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("บันทึกลงโฟลเดอร์ในคอม:").size(13.0).strong().color(NOTION_TEXT_MAIN));
                        if ui.button("เลือกโฟลเดอร์บนคอม...").clicked() {
                            if let Some(f) = rfd::FileDialog::new().pick_folder() {
                                self.pc_folder = Some(f);
                            }
                        }
                    });
                    ui.add_space(4.0);
                    match &self.pc_folder {
                        Some(f) => {
                            ui.add(egui::Label::new(RichText::new(f.to_string_lossy()).size(12.0).color(Color32::from_rgb(100, 220, 150))).wrap());
                        }
                        None => {
                            ui.label(RichText::new("ยังไม่ได้เลือกโฟลเดอร์ปลายทางบนคอม").size(12.0).color(Color32::from_rgb(230, 160, 50)));
                        }
                    }
                });
        }

        // 4. ACTION & MODE
        ui.horizontal(|ui| {
            ui.label(RichText::new("การทำงาน:").size(12.0).color(NOTION_TEXT_MUTED));

            let copy_selected = self.mode == Some(OperationMode::Copy);
            let move_selected = self.mode == Some(OperationMode::Move);

            let copy_btn = ui.add_sized(
                [110.0, 28.0],
                egui::Button::new(
                    RichText::new("คัดลอก (Copy)")
                        .size(12.0)
                        .color(if copy_selected { Color32::WHITE } else { NOTION_TEXT_MUTED }),
                )
                .fill(if copy_selected { Color32::from_rgb(45, 75, 110) } else { NOTION_CARD })
                .stroke(Stroke::new(1.0_f32, if copy_selected { NOTION_BLUE } else { NOTION_BORDER })),
            );
            if copy_btn.clicked() {
                self.mode = Some(OperationMode::Copy);
            }

            let move_btn = ui.add_sized(
                [110.0, 28.0],
                egui::Button::new(
                    RichText::new("ย้าย (Move)")
                        .size(12.0)
                        .color(if move_selected { Color32::WHITE } else { NOTION_TEXT_MUTED }),
                )
                .fill(if move_selected { Color32::from_rgb(45, 75, 110) } else { NOTION_CARD })
                .stroke(Stroke::new(1.0_f32, if move_selected { NOTION_BLUE } else { NOTION_BORDER })),
            );
            if move_btn.clicked() {
                self.mode = Some(OperationMode::Move);
            }

            if self.mode.is_none() {
                ui.label(RichText::new("(กรุณาเลือก คัดลอก หรือ ย้าย)").size(11.0).color(Color32::from_rgb(230, 150, 50)));
            }

            ui.add_space(16.0);

            if !self.is_running {
                let action_title = match (self.direction, self.mode) {
                    (TransferDirection::PCtoDrive, Some(OperationMode::Copy)) => "เริ่มคัดลอกลงแฟลชไดรฟ์",
                    (TransferDirection::PCtoDrive, Some(OperationMode::Move)) => "เริ่มย้ายลงแฟลชไดรฟ์",
                    (TransferDirection::DriveToPC, Some(OperationMode::Copy)) => "เริ่มคัดลอกลงคอมพิวเตอร์",
                    (TransferDirection::DriveToPC, Some(OperationMode::Move)) => "เริ่มย้ายลงคอมพิวเตอร์",
                    _ => "เริ่มทำงาน (กรุณาเลือกโหมดก่อน)",
                };

                let can_start = self.mode.is_some() && !self.selected_items.is_empty();
                let start_btn_bg = if can_start { NOTION_BLUE } else { Color32::from_rgb(50, 50, 55) };

                let btn = ui.add_sized(
                    [210.0, 36.0],
                    egui::Button::new(
                        RichText::new(action_title)
                            .size(13.0)
                            .strong()
                            .color(if can_start { Color32::WHITE } else { NOTION_TEXT_MUTED }),
                    )
                    .fill(start_btn_bg),
                );

                if btn.clicked() {
                    self.start_transfer();
                }
            } else {
                let cancel_btn = ui.add_sized(
                    [130.0, 36.0],
                    egui::Button::new(
                        RichText::new("ยกเลิก (Cancel)")
                            .size(13.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(180, 50, 50)),
                );
                if cancel_btn.clicked() {
                    self.cancel_transfer();
                }
            }

            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(&self.status_text).size(12.0).color(NOTION_TEXT_MUTED)).wrap());
        });

        // 5. PROGRESS BAR & REALTIME STATS (เด่นชัด สไตล์ Notion Card)
        if self.is_running || self.progress.is_some() {
            let prog = self.progress.as_ref();
            let total_bytes = prog.map_or(0, |p| p.total_bytes);
            let copied_bytes = prog.map_or(0, |p| p.copied_bytes);
            let total_files = prog.map_or(0, |p| p.total_files);
            let copied_files = prog.map_or(0, |p| p.copied_files);
            let bytes_per_sec = prog.map_or(0.0, |p| p.bytes_per_sec);
            let current_file = prog.map_or("เตรียมการถ่ายโอน...", |p| p.current_file.as_str());

            let frac = if total_bytes > 0 {
                (copied_bytes as f32 / total_bytes as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let speed_mb = bytes_per_sec / (1024.0 * 1024.0);
            let label = format!(
                "{:.1}% ({} / {})",
                frac * 100.0,
                format_bytes(copied_bytes),
                format_bytes(total_bytes)
            );

            Frame::none()
                .fill(Color32::from_rgb(28, 36, 46))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 75, 110)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(14.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let header_text = if self.is_running {
                            format!("กำลังถ่ายโอนข้อมูล... ({:.1}s)", prog.map_or(0.0, |p| p.elapsed_secs))
                        } else {
                            format!("เสร็จสิ้นเรียบร้อย! (ใช้เวลาทั้งหมด {:.2} วินาที)", prog.map_or(0.0, |p| p.elapsed_secs))
                        };

                        ui.label(
                            RichText::new(header_text)
                                .size(13.0)
                                .strong()
                                .color(if self.is_running { Color32::WHITE } else { Color32::from_rgb(100, 225, 140) }),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let speed_title = if self.is_running { "ความเร็ว:" } else { "ความเร็วเฉลี่ย:" };
                            ui.label(
                                RichText::new(format!("{} {:.2} MB/s", speed_title, speed_mb))
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(255, 205, 80)),
                            );
                        });
                    });

                    ui.add_space(6.0);

                    // Animated Progress Bar
                    ui.add(
                        ProgressBar::new(frac)
                            .text(RichText::new(label).strong())
                            .animate(self.is_running),
                    );

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("ความคืบหน้าไฟล์: {} / {}", copied_files, total_files))
                                .size(12.0)
                                .color(NOTION_TEXT_MUTED),
                        );
                        ui.separator();
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("ไฟล์: {}", current_file))
                                    .size(12.0)
                                    .color(NOTION_TEXT_MAIN),
                            )
                            .wrap(),
                        );
                    });
                });
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(760.0, 560.0))
            .with_min_inner_size(Vec2::new(620.0, 450.0))
            .with_title("Fast Transfer"),
        ..Default::default()
    };

    eframe::run_native(
        "fast_transfer",
        options,
        Box::new(|cc| {
            FastTransferApp::setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(FastTransferApp::default()))
        }),
    )
}
