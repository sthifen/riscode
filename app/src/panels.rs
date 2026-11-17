//! Paneles: pantalla bitmap, registros, terminal, problemas, memoria, código máquina,
//! explorador y referencia.

use egui::{Color32, RichText, Sense, Ui, Vec2};
use riscode_asm::lexer::ABI_NAMES;
use riscode_sim::memory::{GP_INIT, HEAP_BASE, KBD_CONTROL, SP_INIT};

use crate::app::{App, BottomTab, ConsoleKind, SideView};
use crate::files::is_asm;
use crate::session::RunState;
use crate::theme;

pub const BASES: &[(u32, &str)] = &[
    (0x1001_0000, "0x10010000 (.data)"),
    (GP_INIT, "0x10008000 (gp)"),
    (0x1000_0000, "0x10000000 (extern)"),
    (HEAP_BASE, "0x10040000 (heap)"),
];

pub fn section_header(ui: &mut Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).size(11.0).strong().color(theme::FG));
    ui.add_space(2.0);
}

impl App {
    // ───────────────────────── Pantalla bitmap ─────────────────────────
    pub fn panel_display(&mut self, ui: &mut Ui) {
        let d = self.settings.display;
        let (w, h) = ((d.width / d.unit_w).max(1) as usize, (d.height / d.unit_h).max(1) as usize);
        let mut pixels = vec![Color32::BLACK; w * h];
        if let Some(s) = &self.session {
            if let Some(mem) = s.machine.mem.slice(d.base, (w * h * 4) as u32) {
                for (i, px) in pixels.iter_mut().enumerate() {
                    let b = &mem[i * 4..i * 4 + 4];
                    *px = Color32::from_rgb(b[2], b[1], b[0]);
                }
            }
        }
        let img = egui::ColorImage::new([w, h], pixels);
        let tex = match &mut self.display_tex {
            Some(t) => {
                t.set(img, egui::TextureOptions::NEAREST);
                t.clone()
            }
            None => {
                let t = ui.ctx().load_texture("pantalla", img, egui::TextureOptions::NEAREST);
                self.display_tex = Some(t.clone());
                t
            }
        };

        ui.horizontal(|ui| {
            section_header(ui, "PANTALLA BITMAP");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⛶").on_hover_text("Ver la pantalla en grande").clicked() {
                    self.big_display = !self.big_display;
                }
                ui.menu_button(RichText::new("⚙").size(13.0), |ui| self.display_settings(ui))
                    .response
                    .on_hover_text("Configurar la pantalla (como el Bitmap Display de RARS)");
                ui.label(
                    RichText::new(format!("{}×{} unidades · {}", w, h, base_name(d.base)))
                        .size(11.0)
                        .color(theme::FG_DIM),
                );
            });
        });
        let avail = ui.available_width();
        let max_h = (ui.available_height() * 0.55).max(120.0);
        let aspect = d.height as f32 / d.width as f32;
        let mut size = Vec2::new(avail, avail * aspect);
        if size.y > max_h {
            size = Vec2::new(max_h / aspect, max_h);
        }
        let resp = ui
            .vertical_centered(|ui| {
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                let border = if self.keyboard_to_program { theme::ACCENT_BRIGHT } else { theme::BORDER };
                ui.painter().rect_stroke(rect, 0.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Outside);
                resp
            })
            .inner;
        if resp.clicked() {
            self.give_keyboard_to_program(ui.ctx());
        }
        resp.on_hover_text("Haz clic aquí para que el teclado vaya al programa");

        if self.big_display {
            let mut open = true;
            egui::Window::new("Pantalla")
                .open(&mut open)
                .default_size(Vec2::new(560.0, 560.0 * aspect + 30.0))
                .resizable(true)
                .show(ui.ctx(), |ui| {
                    let avail = ui.available_size();
                    let mut s = Vec2::new(avail.x, avail.x * aspect);
                    if s.y > avail.y {
                        s = Vec2::new(avail.y / aspect, avail.y);
                    }
                    let (rect, r) = ui.allocate_exact_size(s, Sense::click());
                    ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    if r.clicked() {
                        self.give_keyboard_to_program(ui.ctx());
                    }
                });
            self.big_display = open;
        }
    }

    fn display_settings(&mut self, ui: &mut Ui) {
        let d = &mut self.settings.display;
        egui::Grid::new("display_cfg").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            for (label, val, opts) in [
                ("Ancho de unidad", &mut d.unit_w, &[1u32, 2, 4, 8, 16, 32][..]),
                ("Alto de unidad", &mut d.unit_h, &[1, 2, 4, 8, 16, 32][..]),
                ("Ancho de pantalla", &mut d.width, &[64, 128, 256, 512, 1024][..]),
                ("Alto de pantalla", &mut d.height, &[64, 128, 256, 512, 1024][..]),
            ] {
                ui.label(label);
                egui::ComboBox::from_id_salt(label).selected_text(val.to_string()).show_ui(ui, |ui| {
                    for o in opts {
                        ui.selectable_value(val, *o, o.to_string());
                    }
                });
                ui.end_row();
            }
            ui.label("Dirección base");
            egui::ComboBox::from_id_salt("base").selected_text(base_name(d.base)).show_ui(ui, |ui| {
                for (b, n) in BASES {
                    ui.selectable_value(&mut d.base, *b, *n);
                }
            });
            ui.end_row();
        });
        ui.separator();
        if ui.button("Valores de RARS por defecto (1×1, 512×256, .data)").clicked() {
            *d = crate::app::DisplayCfg { unit_w: 1, unit_h: 1, width: 512, height: 256, base: 0x1001_0000 };
        }
        if ui.button("Como Battle City/Pong (8×8, 512×512, gp)").clicked() {
            *d = crate::app::DisplayCfg::default();
        }
    }

    // ───────────────────────── Registros ─────────────────────────
    pub fn panel_registers(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            section_header(ui, "REGISTROS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.selectable_value(&mut self.settings.regs_hex, false, "Dec");
                ui.selectable_value(&mut self.settings.regs_hex, true, "Hex");
            });
        });
        let (regs, prev, pc) = match &self.session {
            Some(s) => (s.machine.regs, s.prev_regs, s.machine.pc),
            None => {
                let mut r = [0u32; 32];
                r[2] = SP_INIT;
                r[3] = GP_INIT;
                (r, r, riscode_asm::TEXT_BASE)
            }
        };
        let hex = self.settings.regs_hex;
        let fmt = |v: u32| if hex { format!("0x{v:08x}") } else { (v as i32).to_string() };
        egui::ScrollArea::vertical().id_salt("regs").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("regs_grid").num_columns(3).striped(true).spacing([10.0, 2.0]).show(ui, |ui| {
                for i in 0..32 {
                    let changed = regs[i] != prev[i];
                    ui.label(RichText::new(ABI_NAMES[i]).monospace().color(theme::syntax(riscode_asm::highlight::Class::Register)));
                    ui.label(RichText::new(format!("x{i}")).monospace().color(theme::FG_DIM));
                    let mut t = RichText::new(fmt(regs[i])).monospace();
                    if changed {
                        t = t.background_color(theme::CHANGED).color(theme::FG_BRIGHT);
                    }
                    ui.label(t);
                    ui.end_row();
                }
                ui.label(RichText::new("pc").monospace().strong());
                ui.label("");
                ui.label(RichText::new(format!("0x{pc:08x}")).monospace().strong());
                ui.end_row();
            });
        });
    }

    // ───────────────────────── Terminal ─────────────────────────
    pub fn panel_terminal(&mut self, ui: &mut Ui) {
        let waiting = matches!(self.session.as_ref().map(|s| &s.state), Some(RunState::WaitingInput(_)));
        let input_h = 28.0;
        egui::ScrollArea::vertical()
            .id_salt("terminal")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(ui.available_height() - input_h)
            .show(ui, |ui| {
                for l in &self.console {
                    let color = match l.kind {
                        ConsoleKind::Out => theme::FG,
                        ConsoleKind::Info => theme::FG_DIM,
                        ConsoleKind::Error => theme::ERROR,
                        ConsoleKind::Input => theme::syntax(riscode_asm::highlight::Class::String),
                    };
                    ui.label(RichText::new(&l.text).monospace().color(color));
                }
            });
        ui.horizontal(|ui| {
            let prompt = match self.session.as_ref().map(|s| &s.state) {
                Some(RunState::WaitingInput(k)) => match k {
                    riscode_sim::InputKind::Int => "Escribe un número ›",
                    riscode_sim::InputKind::String => "Escribe un texto ›",
                    riscode_sim::InputKind::Char => "Escribe un carácter ›",
                },
                _ => "›",
            };
            ui.label(RichText::new(prompt).monospace().color(if waiting { theme::WARNING } else { theme::FG_DIM }));
            let r = ui.add_enabled(
                waiting,
                egui::TextEdit::singleline(&mut self.console_input).id(egui::Id::new("console_input")).desired_width(f32::INFINITY).font(theme::mono()),
            );
            if waiting && !r.has_focus() && self.focus_console {
                r.request_focus();
                self.focus_console = false;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && waiting {
                let text = std::mem::take(&mut self.console_input);
                self.log(ConsoleKind::Input, text.clone());
                if let Some(s) = &mut self.session {
                    s.machine.provide_input(text);
                    s.state = RunState::Running;
                }
            }
        });
    }

    // ───────────────────────── Problemas ─────────────────────────
    pub fn panel_problems(&mut self, ui: &mut Ui) {
        let Some(doc) = self.docs.get_mut(self.active) else {
            ui.label("No hay archivos abiertos.");
            return;
        };
        if doc.errors.is_empty() {
            ui.label(RichText::new("No hay problemas. ✔").color(theme::FG_DIM));
            return;
        }
        let name = doc.name();
        let mut goto = None;
        egui::ScrollArea::vertical().id_salt("problems").auto_shrink([false, false]).show(ui, |ui| {
            for e in &doc.errors {
                let r = ui.add(
                    egui::Label::new(
                        RichText::new(format!("✖  {}   {}  [línea {}, col {}]", e.msg, name, e.line + 1, e.start + 1))
                            .color(theme::FG),
                    )
                    .sense(Sense::click()),
                );
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if r.clicked() {
                    goto = Some(e.line);
                }
            }
        });
        if let Some(l) = goto {
            doc.goto_line = Some((l, true));
        }
    }

    // ───────────────────────── Memoria ─────────────────────────
    pub fn panel_memory(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Ir a:");
            for (addr, name) in [(0x1001_0000u32, ".data"), (GP_INIT, "gp"), (HEAP_BASE, "heap"), (0xFFFF_0000, "MMIO")] {
                if ui.selectable_label(self.mem_addr == addr, name).clicked() {
                    self.mem_addr = addr;
                    self.mem_addr_text = format!("{addr:08x}");
                }
            }
            if ui.button("sp").clicked() {
                let sp = self.session.as_ref().map(|s| s.machine.regs[2]).unwrap_or(SP_INIT);
                self.mem_addr = sp & !31;
                self.mem_addr_text = format!("{:08x}", self.mem_addr);
            }
            ui.label("0x");
            let r = ui.add(egui::TextEdit::singleline(&mut self.mem_addr_text).desired_width(80.0).font(theme::mono()));
            if r.lost_focus() {
                if let Ok(v) = u32::from_str_radix(self.mem_addr_text.trim().trim_start_matches("0x"), 16) {
                    self.mem_addr = v & !31;
                }
            }
        });
        let Some(s) = &self.session else {
            ui.label(RichText::new("Corre o ensambla el programa para ver la memoria.").color(theme::FG_DIM));
            return;
        };
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
        let base = self.mem_addr;
        let rows = 2048usize;
        egui::ScrollArea::both().id_salt(("memory", base)).auto_shrink([false, false]).show_rows(ui, row_h, rows, |ui, range| {
            for r in range {
                let addr = base.wrapping_add((r * 32) as u32);
                let mut line = format!("0x{addr:08x}  ");
                let mut ascii = String::new();
                for k in 0..8u32 {
                    match s.machine.mem.peek_u32(addr.wrapping_add(k * 4)) {
                        Some(v) => {
                            line.push_str(&format!("{v:08x} "));
                            for b in v.to_le_bytes() {
                                ascii.push(if (0x20..0x7F).contains(&b) { b as char } else { '·' });
                            }
                        }
                        None => line.push_str("-------- "),
                    }
                }
                ui.add(egui::Label::new(RichText::new(format!("{line} {ascii}")).monospace()).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
    }

    // ───────────────────────── Código máquina ─────────────────────────
    pub fn panel_machine_code(&mut self, ui: &mut Ui) {
        let Some(s) = &self.session else {
            ui.label(RichText::new("Ensambla el programa (Ctrl+B) para ver el código máquina.").color(theme::FG_DIM));
            return;
        };
        let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
        let pc_row = ((s.machine.pc.wrapping_sub(riscode_asm::TEXT_BASE)) / 4) as usize;
        let mut sa = egui::ScrollArea::both().id_salt("machine_code").auto_shrink([false, false]);
        let follow = s.state != RunState::Running && self.mc_last_pc != Some(s.machine.pc);
        if follow && pc_row < s.prog.text.len() {
            sa = sa.vertical_scroll_offset((pc_row as f32 * row_h - ui.available_height() / 3.0).max(0.0));
            self.mc_last_pc = Some(s.machine.pc);
        }
        let src_lines = &self.session_src_lines;
        sa.show_rows(ui, row_h, s.prog.text.len(), |ui, range| {
            for i in range {
                let addr = riscode_asm::TEXT_BASE + 4 * i as u32;
                let w = s.prog.text[i];
                let line = s.prog.text_lines[i];
                let src = src_lines.get(line).map(|l| riscode_asm::lexer::strip_comment(l).trim()).unwrap_or("");
                let first_of_line = i == 0 || s.prog.text_lines[i - 1] != line;
                let text = format!(
                    "0x{addr:08x}  {w:08x}  {:<26} {}",
                    riscode_sim::disasm(w, addr),
                    if first_of_line { format!("{:>5}: {src}", line + 1) } else { String::new() }
                );
                let mut rt = RichText::new(text).monospace();
                if i == pc_row && s.is_live() {
                    rt = rt.background_color(theme::EXEC_LINE).color(theme::FG_BRIGHT);
                }
                ui.add(egui::Label::new(rt).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
    }

    // ───────────────────────── Explorador ─────────────────────────
    pub fn panel_explorer(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            section_header(ui, "EXPLORADOR");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("📂").on_hover_text("Abrir carpeta").clicked() {
                    self.cmd_open_folder();
                }
                if ui.small_button("🗋").on_hover_text("Archivo nuevo").clicked() {
                    self.cmd_new();
                }
            });
        });
        let Some(root) = self.explorer.root.clone() else {
            ui.add_space(8.0);
            ui.label(RichText::new("No hay ninguna carpeta abierta.").color(theme::FG_DIM));
            ui.add_space(6.0);
            if ui.button("Abrir carpeta").clicked() {
                self.cmd_open_folder();
            }
            return;
        };
        let name = root.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default();
        ui.label(RichText::new(name).strong().size(12.0));
        let mut open = None;
        egui::ScrollArea::vertical().id_salt("explorer").auto_shrink([false, false]).show(ui, |ui| {
            self.tree(ui, &root, 1, &mut open);
        });
        if let Some(p) = open {
            self.open_path(&p);
        }
    }

    fn tree(&mut self, ui: &mut Ui, dir: &std::path::Path, depth: usize, open: &mut Option<std::path::PathBuf>) {
        for e in self.explorer.list_cached(dir) {
            let indent = depth as f32 * 12.0;
            let active = self.docs.get(self.active).and_then(|d| d.path.as_ref()) == Some(&e.path);
            let label = if e.is_dir {
                let icon = if self.explorer.expanded.contains(&e.path) { "🗁" } else { "🗀" };
                format!("{icon} {}", e.name)
            } else {
                let icon = if is_asm(&e.path) { "⚙" } else { "🗋" };
                format!("  {icon} {}", e.name)
            };
            let color = if is_asm(&e.path) || e.is_dir { theme::FG } else { theme::FG_DIM };
            let r = ui
                .horizontal(|ui| {
                    ui.add_space(indent);
                    ui.selectable_label(active, RichText::new(label).color(color))
                })
                .inner;
            if r.clicked() {
                if e.is_dir {
                    if !self.explorer.expanded.remove(&e.path) {
                        self.explorer.expanded.insert(e.path.clone());
                    }
                } else {
                    *open = Some(e.path.clone());
                }
            }
            if e.is_dir && self.explorer.expanded.contains(&e.path) && depth < 12 {
                self.tree(ui, &e.path, depth + 1, open);
            }
        }
    }

    // ───────────────────────── Referencia ─────────────────────────
    pub fn panel_reference(&mut self, ui: &mut Ui) {
        section_header(ui, "REFERENCIA RISC-V");
        ui.add(egui::TextEdit::singleline(&mut self.ref_filter).hint_text("Buscar instrucción…"));
        let f = self.ref_filter.to_lowercase();
        egui::ScrollArea::vertical().id_salt("reference").auto_shrink([false, false]).show(ui, |ui| {
            let groups: [(&str, Vec<&str>); 4] = [
                ("Instrucciones", riscode_asm::instr::INSTRUCCIONES.iter().map(|i| i.nombre).collect()),
                ("Pseudoinstrucciones", riscode_asm::instr::PSEUDO.to_vec()),
                ("Directivas", riscode_asm::instr::DIRECTIVAS.to_vec()),
                ("", vec![]),
            ];
            for (title, names) in groups.iter().filter(|g| !g.0.is_empty()) {
                let items: Vec<_> = names
                    .iter()
                    .filter_map(|n| riscode_asm::instr::documentacion(n).map(|d| (n, d)))
                    .filter(|(n, d)| f.is_empty() || n.contains(&f) || d.1.to_lowercase().contains(&f))
                    .collect();
                if items.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(*title).strong()).default_open(true).id_salt(title).show(ui, |ui| {
                    for (_, (syn, desc)) in items {
                        ui.label(RichText::new(syn).monospace().color(theme::syntax(riscode_asm::highlight::Class::Instruction)));
                        ui.label(RichText::new(desc).size(12.0).color(theme::FG_DIM));
                        ui.add_space(3.0);
                    }
                });
            }
            if f.is_empty() || "syscall ecall".contains(&f) {
                egui::CollapsingHeader::new(RichText::new("Llamadas al sistema (a7)").strong()).default_open(true).show(ui, |ui| {
                    egui::Grid::new("syscalls").striped(true).show(ui, |ui| {
                        for (n, d) in SYSCALLS {
                            ui.label(RichText::new(n.to_string()).monospace().color(theme::syntax(riscode_asm::highlight::Class::Number)));
                            ui.label(RichText::new(*d).size(12.0));
                            ui.end_row();
                        }
                    });
                });
                egui::CollapsingHeader::new(RichText::new("Teclado MMIO").strong()).default_open(false).show(ui, |ui| {
                    ui.label(RichText::new(format!("0x{KBD_CONTROL:08x}  bit 0 = hay tecla")).monospace());
                    ui.label(RichText::new("0xffff0004  código ASCII de la tecla").monospace());
                    ui.label(RichText::new("0xffff000c  escribir un carácter en la terminal").monospace());
                });
            }
        });
    }

    pub fn side_view_button(&mut self, ui: &mut Ui, view: SideView, icon: &str, tip: &str) {
        let active = self.settings.show_side && self.side == view;
        let color = if active { theme::FG_BRIGHT } else { theme::FG_DIM };
        let (rect, r) = ui.allocate_exact_size(Vec2::new(48.0, 44.0), Sense::click());
        if active {
            ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 0.0, theme::FG_BRIGHT);
        }
        let c = if r.hovered() { theme::FG_BRIGHT } else { color };
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(22.0), c);
        if r.clicked() {
            if self.side == view && self.settings.show_side {
                self.settings.show_side = false;
            } else {
                self.side = view;
                self.settings.show_side = true;
            }
        }
        r.on_hover_text(tip);
    }

    pub fn bottom_tabs(&mut self, ui: &mut Ui) {
        let n_err = self.docs.get(self.active).map(|d| d.errors.len()).unwrap_or(0);
        ui.horizontal(|ui| {
            for (tab, name) in [
                (BottomTab::Terminal, "TERMINAL".to_string()),
                (BottomTab::Problems, if n_err > 0 { format!("PROBLEMAS ({n_err})") } else { "PROBLEMAS".into() }),
                (BottomTab::Memory, "MEMORIA".into()),
                (BottomTab::MachineCode, "CÓDIGO MÁQUINA".into()),
            ] {
                let active = self.bottom_tab == tab;
                let color = if active { theme::FG_BRIGHT } else { theme::FG_DIM };
                let r = ui.add(egui::Label::new(RichText::new(name).size(11.0).color(color)).sense(Sense::click()));
                if active {
                    let rr = r.rect;
                    ui.painter().hline(rr.x_range(), rr.bottom() + 3.0, egui::Stroke::new(1.0, theme::FG_BRIGHT));
                }
                if r.clicked() {
                    self.bottom_tab = tab;
                }
                ui.add_space(12.0);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text("Cerrar panel").clicked() {
                    self.settings.show_bottom = false;
                }
                if self.bottom_tab == BottomTab::Terminal && ui.small_button("🗑").on_hover_text("Limpiar terminal").clicked() {
                    self.console.clear();
                }
            });
        });
        ui.add_space(4.0);
    }
}

fn base_name(b: u32) -> &'static str {
    BASES.iter().find(|x| x.0 == b).map(|x| x.1).unwrap_or("otra")
}

pub const SYSCALLS: &[(u32, &str)] = &[
    (1, "Imprimir entero (a0)"),
    (4, "Imprimir texto (a0 = dirección)"),
    (5, "Leer entero → a0"),
    (8, "Leer texto (a0 = buffer, a1 = tamaño)"),
    (9, "sbrk: reservar a0 bytes → a0 = dirección"),
    (10, "Salir"),
    (11, "Imprimir carácter (a0)"),
    (12, "Leer carácter → a0"),
    (17, "Salir con código (a0)"),
    (30, "Hora en ms → a0 (bajo), a1 (alto)"),
    (32, "Dormir a0 milisegundos"),
    (34, "Imprimir entero en hexadecimal"),
    (35, "Imprimir entero en binario"),
    (36, "Imprimir entero sin signo"),
    (40, "Semilla aleatoria (a0 = id, a1 = semilla)"),
    (41, "Entero aleatorio → a0"),
    (42, "Aleatorio en [0, a1) → a0"),
];
