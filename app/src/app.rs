//! La aplicación: estado, comandos (abrir, guardar, correr…) y la disposición estilo VS Code.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Key, KeyboardShortcut, Modifiers, RichText, Ui};
use serde::{Deserialize, Serialize};

use crate::editor::{self, EditorContext, HighlightCache};
use crate::files::{Document, Explorer};
use crate::session::{RunState, Session, Speed};
use crate::theme;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayCfg {
    pub unit_w: u32,
    pub unit_h: u32,
    pub width: u32,
    pub height: u32,
    pub base: u32,
}

impl Default for DisplayCfg {
    /// Lo que usan los proyectos del curso: unidades 8×8, 512×512, base en gp.
    fn default() -> Self {
        DisplayCfg { unit_w: 8, unit_h: 8, width: 512, height: 512, base: riscode_sim::memory::GP_INIT }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub workspace: Option<PathBuf>,
    pub open_files: Vec<PathBuf>,
    pub active: usize,
    pub recent: Vec<PathBuf>,
    pub display: DisplayCfg,
    pub speed: Speed,
    pub regs_hex: bool,
    pub show_side: bool,
    pub show_bottom: bool,
    pub show_right: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            workspace: None,
            open_files: vec![],
            active: 0,
            recent: vec![],
            display: DisplayCfg::default(),
            speed: Speed::Max,
            regs_hex: true,
            show_side: true,
            show_bottom: true,
            show_right: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideView {
    Explorer,
    Reference,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Terminal,
    Problems,
    Memory,
    MachineCode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConsoleKind {
    Out,
    Info,
    Error,
    Input,
}

pub struct ConsoleLine {
    pub text: String,
    pub kind: ConsoleKind,
}

/// Opciones de la línea de comandos (las de prueba sirven para revisar la app automáticamente).
#[derive(Default)]
pub struct Startup {
    pub files: Vec<PathBuf>,
    pub run: bool,
    pub step: u32,
    pub panel: Option<String>,
    pub keys: Vec<(u64, u8)>,
    pub screenshot: Option<(PathBuf, u64)>,
}

enum Pending {
    CloseTab(usize),
    Quit,
}

pub struct App {
    pub docs: Vec<Document>,
    pub active: usize,
    caches: HashMap<u64, HighlightCache>,
    pub explorer: Explorer,
    pub session: Option<Session>,
    pub session_src_lines: Vec<String>,
    pub console: Vec<ConsoleLine>,
    pub console_input: String,
    pub focus_console: bool,
    out_open: bool,
    pub settings: Settings,
    pub display_tex: Option<egui::TextureHandle>,
    pub big_display: bool,
    pub side: SideView,
    pub bottom_tab: BottomTab,
    pub mem_addr: u32,
    pub mem_addr_text: String,
    pub mc_last_pc: Option<u32>,
    pub ref_filter: String,
    pub keyboard_to_program: bool,
    last_exec_line: Option<usize>,
    pending: Option<Pending>,
    allow_close: bool,
    startup: Startup,
    started: Instant,
    last_frame: Instant,
    screenshot_requested: bool,
    toast: Option<(String, Instant)>,
    last_title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
        theme::apply(&cc.egui_ctx);
        let settings: Settings = cc.storage.and_then(|s| eframe::get_value(s, "riscode")).unwrap_or_default();
        let mut app = App {
            docs: vec![],
            active: 0,
            caches: HashMap::new(),
            explorer: Explorer { root: None, expanded: HashSet::new(), cache: HashMap::new() },
            session: None,
            session_src_lines: vec![],
            console: vec![],
            console_input: String::new(),
            focus_console: false,
            out_open: false,
            settings,
            display_tex: None,
            big_display: false,
            side: SideView::Explorer,
            bottom_tab: BottomTab::Terminal,
            mem_addr: 0x1001_0000,
            mem_addr_text: "10010000".into(),
            mc_last_pc: None,
            ref_filter: String::new(),
            keyboard_to_program: false,
            last_exec_line: None,
            pending: None,
            allow_close: false,
            startup,
            started: Instant::now(),
            last_frame: Instant::now(),
            screenshot_requested: false,
            toast: None,
            last_title: String::new(),
        };
        app.explorer.root = app.settings.workspace.clone().filter(|p| p.is_dir());
        if app.startup.files.is_empty() {
            for p in app.settings.open_files.clone() {
                if let Ok(d) = Document::open(&p) {
                    app.docs.push(d);
                }
            }
            app.active = app.settings.active.min(app.docs.len().saturating_sub(1));
        } else {
            // Al abrir archivos desde la terminal, el explorador muestra la carpeta del primero.
            app.explorer.root = app.startup.files[0].parent().map(|p| p.to_path_buf());
            for p in app.startup.files.clone() {
                app.open_path(&p);
            }
        }
        app.log(ConsoleKind::Info, "RISCode listo. F5 = correr, F10 = paso a paso, Shift+F5 = detener.".into());
        app
    }

    pub fn log(&mut self, kind: ConsoleKind, text: String) {
        self.out_open = false;
        self.console.push(ConsoleLine { text, kind });
        if self.console.len() > 5000 {
            self.console.drain(..1000);
        }
    }

    /// El programa imprimió algo: se pega a la última línea hasta que llegue un salto de línea.
    fn program_output(&mut self, s: &str) {
        let mut parts = s.split('\n').peekable();
        let mut first = true;
        while let Some(part) = parts.next() {
            let is_last = parts.peek().is_none();
            let can_append = first && self.out_open && self.console.last().is_some_and(|l| l.kind == ConsoleKind::Out);
            if can_append {
                self.console.last_mut().unwrap().text.push_str(part);
            } else if !(is_last && part.is_empty()) {
                self.console.push(ConsoleLine { text: part.to_string(), kind: ConsoleKind::Out });
            }
            if is_last {
                self.out_open = !part.is_empty() || (first && self.out_open);
            }
            first = false;
        }
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    // ───────────────────────── archivos ─────────────────────────
    pub fn open_path(&mut self, p: &Path) {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(p)) {
            self.active = i;
            return;
        }
        match Document::open(p) {
            Ok(d) => {
                if d.was_latin1 {
                    self.log(ConsoleKind::Info, format!("{} estaba en Latin-1; se guardará como UTF-8.", d.name()));
                }
                // si la pestaña actual es un "sin título" vacío, se reemplaza
                if self.docs.get(self.active).is_some_and(|d| d.path.is_none() && d.text.is_empty()) {
                    self.docs[self.active] = d;
                } else {
                    self.docs.push(d);
                    self.active = self.docs.len() - 1;
                }
                self.settings.recent.retain(|r| r != p);
                self.settings.recent.insert(0, p.to_path_buf());
                self.settings.recent.truncate(10);
            }
            Err(e) => self.log(ConsoleKind::Error, format!("No se pudo abrir {}: {e}", p.display())),
        }
    }

    pub fn cmd_new(&mut self) {
        let mut d = Document::new_empty();
        d.text = "# Programa nuevo\n.data\n\n.text\nmain:\n\tli a0, 42\n\tli a7, 1\n\tecall\n\n\tli a7, 10\n\tecall\n".into();
        self.docs.push(d);
        self.active = self.docs.len() - 1;
    }

    fn cmd_open_file(&mut self) {
        let mut dlg = rfd::FileDialog::new().add_filter("Ensamblador RISC-V", &["asm", "s", "S"]).add_filter("Todos", &["*"]);
        if let Some(r) = &self.explorer.root {
            dlg = dlg.set_directory(r);
        }
        if let Some(p) = dlg.pick_file() {
            self.open_path(&p);
        }
    }

    pub fn cmd_open_folder(&mut self) {
        if let Some(p) = rfd::FileDialog::new().pick_folder() {
            self.explorer.root = Some(p.clone());
            self.settings.workspace = Some(p);
            self.side = SideView::Explorer;
            self.settings.show_side = true;
        }
    }

    fn cmd_save(&mut self, idx: usize, save_as: bool) -> bool {
        let Some(doc) = self.docs.get_mut(idx) else { return false };
        if doc.path.is_none() || save_as {
            let mut dlg = rfd::FileDialog::new().add_filter("Ensamblador RISC-V", &["asm", "s"]).set_file_name(doc.name());
            if let Some(r) = &self.explorer.root {
                dlg = dlg.set_directory(r);
            }
            match dlg.save_file() {
                Some(p) => doc.path = Some(p),
                None => return false,
            }
        }
        match doc.save() {
            Ok(()) => {
                let name = doc.name();
                self.toast(format!("Guardado {name}"));
                true
            }
            Err(e) => {
                let msg = format!("No se pudo guardar: {e}");
                self.log(ConsoleKind::Error, msg);
                false
            }
        }
    }

    fn close_tab(&mut self, idx: usize, force: bool) {
        if idx >= self.docs.len() {
            return;
        }
        if self.docs[idx].dirty && !force {
            self.pending = Some(Pending::CloseTab(idx));
            return;
        }
        let d = self.docs.remove(idx);
        self.caches.remove(&d.id);
        if self.session.as_ref().is_some_and(|s| s.doc_id == d.id) {
            self.session = None;
        }
        if self.active >= self.docs.len() {
            self.active = self.docs.len().saturating_sub(1);
        } else if self.active > idx {
            self.active -= 1;
        }
    }

    fn cmd_export(&mut self, data: bool, fmt: riscode_asm::Formato) {
        let Some(doc) = self.docs.get_mut(self.active) else { return };
        let prog = match riscode_asm::assemble(&doc.text) {
            Ok(p) => p,
            Err(e) => {
                doc.errors = e;
                self.bottom_tab = BottomTab::Problems;
                return;
            }
        };
        let words = if data { riscode_asm::palabras_de_datos(&prog.data) } else { prog.text.clone() };
        let out = riscode_asm::exportar_palabras(words, fmt);
        let stem = doc.name().trim_end_matches(".asm").to_string();
        let ext = if fmt == riscode_asm::Formato::Hex { "hex" } else { "bin" };
        let name = format!("{stem}_{}.{ext}", if data { "data" } else { "text" });
        let mut dlg = rfd::FileDialog::new().set_file_name(&name);
        if let Some(p) = doc.path.as_ref().and_then(|p| p.parent()) {
            dlg = dlg.set_directory(p);
        }
        if let Some(p) = dlg.save_file() {
            match std::fs::write(&p, out) {
                Ok(()) => self.toast(format!("Exportado {}", p.display())),
                Err(e) => self.log(ConsoleKind::Error, format!("No se pudo exportar: {e}")),
            }
        }
    }

    // ───────────────────────── ejecución ─────────────────────────
    fn assemble_active(&mut self) -> Option<riscode_asm::Program> {
        let doc = self.docs.get_mut(self.active)?;
        let name = doc.name();
        match riscode_asm::assemble(&doc.text) {
            Ok(p) => {
                doc.errors.clear();
                let msg = format!("Ensamblado {name}: {} instrucciones, {} bytes de datos.", p.text.len(), p.data.len());
                self.log(ConsoleKind::Info, msg);
                Some(p)
            }
            Err(errs) => {
                let first = errs[0].line;
                let n = errs.len();
                doc.errors = errs;
                doc.goto_line = Some((first, true));
                self.bottom_tab = BottomTab::Problems;
                self.settings.show_bottom = true;
                self.log(ConsoleKind::Error, format!("{name}: {n} error(es) al ensamblar. Míralos en PROBLEMAS."));
                None
            }
        }
    }

    fn session_is_fresh(&self) -> bool {
        match (&self.session, self.docs.get(self.active)) {
            (Some(s), Some(d)) => s.is_live() && s.doc_id == d.id && s.doc_version == d.version,
            _ => false,
        }
    }

    fn start_session(&mut self, state: RunState) -> bool {
        let Some(prog) = self.assemble_active() else { return false };
        let doc = &self.docs[self.active];
        self.session_src_lines = doc.text.split('\n').map(|s| s.to_string()).collect();
        self.session = Some(Session::new(prog, doc.id, doc.version, state));
        self.mc_last_pc = None;
        self.last_exec_line = None;
        true
    }

    pub fn give_keyboard_to_program(&mut self, ctx: &egui::Context) {
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }

    fn cmd_run(&mut self, ctx: &egui::Context) {
        if self.session_is_fresh() {
            if let Some(s) = &mut self.session {
                s.resume();
            }
        } else if self.start_session(RunState::Running) {
            let name = self.docs[self.active].name();
            self.log(ConsoleKind::Info, format!("── Ejecutando {name} ──"));
        } else {
            return;
        }
        self.give_keyboard_to_program(ctx);
    }

    fn cmd_step(&mut self) {
        if self.session_is_fresh() {
            if let Some(s) = &mut self.session {
                if matches!(s.state, RunState::Paused | RunState::Running) {
                    s.step();
                }
            }
        } else if self.start_session(RunState::Paused) {
            self.log(ConsoleKind::Info, "── Paso a paso: F10 ejecuta la siguiente instrucción ──".into());
        }
    }

    fn cmd_pause(&mut self) {
        if let Some(s) = &mut self.session {
            if s.state == RunState::Running {
                s.state = RunState::Paused;
            }
        }
    }

    fn cmd_stop(&mut self) {
        if let Some(s) = &mut self.session {
            if s.is_live() {
                s.state = RunState::Finished(-1);
                self.log(ConsoleKind::Info, "── Detenido ──".into());
            }
        }
    }

    fn cmd_assemble(&mut self) {
        if self.start_session(RunState::Paused) {
            self.toast("Ensamblado sin errores");
        }
    }

    fn breakpoint_addrs(&self) -> HashSet<u32> {
        let mut set = HashSet::new();
        let (Some(s), Some(d)) = (&self.session, self.docs.iter().find(|d| Some(d.id) == self.session.as_ref().map(|s| s.doc_id))) else {
            return set;
        };
        for &l in &d.breakpoints {
            // si la línea no tiene código (etiqueta, comentario), se usa la siguiente que sí tenga
            if let Some(a) = (l..(l + 64).min(s.prog.line_addr.len())).find_map(|k| s.prog.line_addr[k]) {
                set.insert(a);
            }
        }
        set
    }

    fn tick_session(&mut self, ctx: &egui::Context, dt: f32) {
        let bps = self.breakpoint_addrs();
        let speed = self.settings.speed;
        let Some(s) = &mut self.session else { return };
        let before = s.state.clone();
        let wait = s.tick(&bps, speed, dt);
        let out = std::mem::take(&mut s.machine.output);
        let tx = std::mem::take(&mut s.machine.mem.tx_out);
        let after = s.state.clone();
        if !out.is_empty() {
            self.program_output(&out);
        }
        if !tx.is_empty() {
            self.program_output(&String::from_utf8_lossy(&tx));
        }
        if before != after {
            match &after {
                RunState::Finished(c) if *c >= 0 => self.log(ConsoleKind::Info, format!("── El programa terminó (código {c}) ──")),
                RunState::Error(e) => {
                    let e = e.clone();
                    self.log(ConsoleKind::Error, format!("Error en ejecución: {e}"));
                    self.settings.show_bottom = true;
                    self.bottom_tab = BottomTab::Terminal;
                }
                RunState::Paused => self.log(ConsoleKind::Info, "── Pausado en un punto de parada ──".into()),
                RunState::WaitingInput(_) => {
                    self.focus_console = true;
                    self.settings.show_bottom = true;
                    self.bottom_tab = BottomTab::Terminal;
                }
                _ => {}
            }
        }
        if let Some(w) = wait {
            if w.is_zero() {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(w);
            }
        }
    }

    fn exec_line(&self) -> Option<(usize, bool)> {
        let s = self.session.as_ref()?;
        let show = match &s.state {
            RunState::Paused | RunState::WaitingInput(_) => true,
            RunState::Error(_) => true,
            RunState::Running => matches!(self.settings.speed, Speed::Limited(ips) if ips <= 50.0),
            RunState::Finished(_) => false,
        };
        if !show {
            return None;
        }
        Some((s.current_line()?, matches!(s.state, RunState::Error(_))))
    }

    fn route_keys_to_program(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let live = self.session.as_ref().is_some_and(|s| s.is_live());
        self.keyboard_to_program = live && !typing;
        if !self.keyboard_to_program {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let Some(s) = &mut self.session else { return };
        for e in events {
            match e {
                egui::Event::Text(t) => {
                    for c in t.chars().filter(|c| c.is_ascii()) {
                        s.machine.key_press(c as u8);
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.is_none() => match key {
                    Key::Enter => s.machine.key_press(b'\n'),
                    Key::Backspace => s.machine.key_press(8),
                    Key::Escape => s.machine.key_press(27),
                    Key::Tab => s.machine.key_press(b'\t'),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m: Modifiers, k: Key| KeyboardShortcut::new(m, k);
        let cmd = Modifiers::COMMAND;
        let pressed = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if pressed(sc(Modifiers::SHIFT, Key::F5)) {
            self.cmd_stop();
        }
        if pressed(sc(Modifiers::NONE, Key::F5)) {
            self.cmd_run(ctx);
        }
        if pressed(sc(Modifiers::NONE, Key::F6)) {
            self.cmd_pause();
        }
        if pressed(sc(Modifiers::NONE, Key::F10)) || pressed(sc(Modifiers::NONE, Key::F7)) {
            self.cmd_step();
        }
        if pressed(sc(Modifiers::NONE, Key::F9)) {
            if let Some(d) = self.docs.get_mut(self.active) {
                let l = d.cursor.0;
                if !d.breakpoints.remove(&l) {
                    d.breakpoints.insert(l);
                }
            }
        }
        if pressed(sc(cmd, Key::B)) {
            self.cmd_assemble();
        }
        if pressed(sc(cmd | Modifiers::SHIFT, Key::S)) {
            self.cmd_save(self.active, true);
        } else if pressed(sc(cmd, Key::S)) {
            self.cmd_save(self.active, false);
        }
        if pressed(sc(cmd | Modifiers::SHIFT, Key::O)) {
            self.cmd_open_folder();
        } else if pressed(sc(cmd, Key::O)) {
            self.cmd_open_file();
        }
        if pressed(sc(cmd, Key::N)) {
            self.cmd_new();
        }
        if pressed(sc(cmd, Key::W)) {
            self.close_tab(self.active, false);
        }
        if pressed(sc(cmd, Key::J)) {
            self.settings.show_bottom = !self.settings.show_bottom;
        }
        if pressed(sc(cmd, Key::Tab)) && !self.docs.is_empty() {
            self.active = (self.active + 1) % self.docs.len();
        }
    }

    // ───────────────────────── interfaz ─────────────────────────
    fn top_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Archivo", |ui| {
                if ui.add(egui::Button::new("Archivo nuevo").shortcut_text("Ctrl+N")).clicked() {
                    self.cmd_new();
                }
                if ui.add(egui::Button::new("Abrir archivo…").shortcut_text("Ctrl+O")).clicked() {
                    self.cmd_open_file();
                }
                if ui.add(egui::Button::new("Abrir carpeta…").shortcut_text("Ctrl+Shift+O")).clicked() {
                    self.cmd_open_folder();
                }
                ui.menu_button("Abrir reciente", |ui| {
                    for p in self.settings.recent.clone() {
                        if ui.button(p.display().to_string()).clicked() {
                            self.open_path(&p);
                        }
                    }
                });
                ui.separator();
                if ui.add(egui::Button::new("Guardar").shortcut_text("Ctrl+S")).clicked() {
                    self.cmd_save(self.active, false);
                }
                if ui.add(egui::Button::new("Guardar como…").shortcut_text("Ctrl+Shift+S")).clicked() {
                    self.cmd_save(self.active, true);
                }
                ui.menu_button("Exportar código máquina (como el Proyecto 3)", |ui| {
                    use riscode_asm::Formato::*;
                    for (label, data, fmt) in [
                        (".text en hexadecimal", false, Hex),
                        (".text en binario", false, Bin),
                        (".data en hexadecimal", true, Hex),
                        (".data en binario", true, Bin),
                    ] {
                        if ui.button(label).clicked() {
                            self.cmd_export(data, fmt);
                        }
                    }
                });
                ui.separator();
                if ui.add(egui::Button::new("Cerrar pestaña").shortcut_text("Ctrl+W")).clicked() {
                    self.close_tab(self.active, false);
                }
                if ui.button("Salir").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Ejecutar", |ui| {
                if ui.add(egui::Button::new("Correr / Continuar").shortcut_text("F5")).clicked() {
                    self.cmd_run(&ctx);
                }
                if ui.add(egui::Button::new("Pausar").shortcut_text("F6")).clicked() {
                    self.cmd_pause();
                }
                if ui.add(egui::Button::new("Paso a paso").shortcut_text("F10")).clicked() {
                    self.cmd_step();
                }
                if ui.add(egui::Button::new("Detener").shortcut_text("Shift+F5")).clicked() {
                    self.cmd_stop();
                }
                ui.separator();
                if ui.add(egui::Button::new("Solo ensamblar").shortcut_text("Ctrl+B")).clicked() {
                    self.cmd_assemble();
                }
                if ui.add(egui::Button::new("Poner/quitar punto de parada").shortcut_text("F9")).clicked() {
                    if let Some(d) = self.docs.get_mut(self.active) {
                        let l = d.cursor.0;
                        if !d.breakpoints.remove(&l) {
                            d.breakpoints.insert(l);
                        }
                    }
                }
                if ui.button("Quitar todos los puntos de parada").clicked() {
                    for d in &mut self.docs {
                        d.breakpoints.clear();
                    }
                }
            });
            ui.menu_button("Ver", |ui| {
                ui.checkbox(&mut self.settings.show_side, "Barra lateral");
                ui.checkbox(&mut self.settings.show_bottom, "Panel inferior (Ctrl+J)");
                ui.checkbox(&mut self.settings.show_right, "Pantalla y registros");
                ui.separator();
                if ui.button("Explorador").clicked() {
                    self.side = SideView::Explorer;
                    self.settings.show_side = true;
                }
                if ui.button("Referencia RISC-V").clicked() {
                    self.side = SideView::Reference;
                    self.settings.show_side = true;
                }
            });
            ui.menu_button("Ayuda", |ui| {
                ui.label(RichText::new("RISCode").strong().size(16.0));
                ui.label("Editor y simulador RISC-V (RV32IM), compatible con RARS.");
                ui.label("Basado en el ensamblador del Proyecto 3 de");
                ui.label("Steven e Isabel Sevilla (TEC).");
                ui.separator();
                ui.label("F5 correr · F6 pausar · F10 paso · Shift+F5 detener");
                ui.label("F9 punto de parada · Ctrl+B ensamblar · Ctrl+J panel");
                ui.label("Clic en el número de línea = punto de parada");
            });

            ui.add_space(24.0);
            self.run_controls(ui);
        });
    }

    fn run_controls(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let state = self.session.as_ref().map(|s| s.state.clone());
        let fresh = self.session_is_fresh();
        let running = state == Some(RunState::Running);
        let live = self.session.as_ref().is_some_and(|s| s.is_live());
        let has_doc = !self.docs.is_empty();
        let btn = |ui: &mut Ui, enabled: bool, icon: &str, color: egui::Color32, tip: &str| {
            ui.add_enabled(enabled, egui::Button::new(RichText::new(icon).color(color).size(15.0)).frame(false)).on_hover_text(tip).clicked()
        };
        egui::Frame::new().fill(theme::SIDEBAR_BG).corner_radius(4.0).inner_margin(egui::Margin::symmetric(6, 1)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = if fresh && state == Some(RunState::Paused) { "Continuar (F5)" } else { "Correr (F5)" };
                if btn(ui, has_doc && !running, "▶", theme::syntax(riscode_asm::highlight::Class::Comment), label) {
                    self.cmd_run(&ctx);
                }
                if btn(ui, running, "⏸", theme::syntax(riscode_asm::highlight::Class::Register), "Pausar (F6)") {
                    self.cmd_pause();
                }
                if btn(ui, has_doc && !running, "⏭", theme::syntax(riscode_asm::highlight::Class::Register), "Paso a paso (F10)") {
                    self.cmd_step();
                }
                if btn(ui, live, "⏹", theme::ERROR, "Detener (Shift+F5)") {
                    self.cmd_stop();
                }
                if btn(ui, has_doc, "🔨", theme::FG, "Solo ensamblar (Ctrl+B)") {
                    self.cmd_assemble();
                }
            });
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Velocidad").color(theme::FG_DIM).size(12.0));
        let speed_name = |s: Speed| match s {
            Speed::Max => "Máxima".to_string(),
            Speed::Limited(v) => format!("{v} instr/s"),
        };
        egui::ComboBox::from_id_salt("speed").selected_text(speed_name(self.settings.speed)).width(96.0).show_ui(ui, |ui| {
            for s in [Speed::Max, Speed::Limited(10000.0), Speed::Limited(1000.0), Speed::Limited(100.0), Speed::Limited(30.0), Speed::Limited(10.0), Speed::Limited(3.0), Speed::Limited(1.0)] {
                ui.selectable_value(&mut self.settings.speed, s, speed_name(s));
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let white = |t: String| RichText::new(t).color(theme::FG_BRIGHT).size(12.0);
            let text = match self.session.as_ref().map(|s| (&s.state, s)) {
                None => match self.docs.get(self.active).map(|d| d.errors.len()) {
                    Some(n) if n > 0 => format!("✖ {n} error(es) al ensamblar"),
                    _ => "Listo".to_string(),
                },
                Some((RunState::Running, s)) => format!("▶ Ejecutando · {:.1} M instr/s", s.ips / 1e6),
                Some((RunState::Paused, s)) => format!("⏸ Pausado en la línea {}", s.current_line().map(|l| l + 1).unwrap_or(0)),
                Some((RunState::WaitingInput(_), _)) => "⌨ Esperando entrada en la terminal".to_string(),
                Some((RunState::Finished(c), s)) if *c >= 0 => format!("■ Terminó (código {c}) · {} instrucciones", s.machine.instr_count),
                Some((RunState::Finished(_), _)) => "■ Detenido".to_string(),
                Some((RunState::Error(e), _)) => format!("✖ {e}"),
            };
            ui.label(white(text));
            if let Some(s) = &self.session {
                if s.doc_version != self.docs.iter().find(|d| d.id == s.doc_id).map(|d| d.version).unwrap_or(0) && s.is_live() {
                    ui.label(white("⚠ El código cambió: F5 lo vuelve a ensamblar".into()));
                }
            }
            if let Some((msg, t)) = &self.toast {
                if t.elapsed() < Duration::from_secs(3) {
                    ui.label(white(format!("✔ {msg}")));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(white("RISC-V RV32IM".into()));
                if let Some(d) = self.docs.get(self.active) {
                    ui.label(white(format!("Ln {}, Col {}", d.cursor.0 + 1, d.cursor.1 + 1)));
                    let n = d.errors.len();
                    if n > 0 {
                        ui.label(white(format!("✖ {n}")));
                    }
                }
                if self.keyboard_to_program {
                    ui.label(white("⌨ El teclado va al programa".into()));
                } else if self.session.as_ref().is_some_and(|s| s.is_live()) {
                    ui.label(white("⌨ El teclado va al editor (clic en la pantalla para jugar)".into()));
                }
            });
        });
    }

    fn tabs(&mut self, ui: &mut Ui) {
        let mut close = None;
        egui::ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                for (i, d) in self.docs.iter().enumerate() {
                    let active = i == self.active;
                    let fill = if active { theme::EDITOR_BG } else { theme::TAB_INACTIVE };
                    let r = egui::Frame::new().fill(fill).inner_margin(egui::Margin { left: 12, right: 6, top: 7, bottom: 7 }).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let color = if active { theme::FG_BRIGHT } else { theme::FG_DIM };
                            let icon_c = theme::syntax(riscode_asm::highlight::Class::Instruction);
                            ui.label(RichText::new("⚙").color(icon_c).size(12.0));
                            let mut name = RichText::new(d.name()).color(color);
                            if !d.errors.is_empty() {
                                name = name.color(theme::ERROR);
                            }
                            ui.label(name);
                            let x = if d.dirty { "●" } else { "✖" };
                            if ui.add(egui::Button::new(RichText::new(x).size(11.0).color(theme::FG_DIM)).frame(false)).clicked() {
                                close = Some(i);
                            }
                        });
                    });
                    let rect = r.response.rect;
                    if active {
                        ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(2.0, theme::ACCENT_BRIGHT));
                    }
                    let resp = ui.interact(rect, egui::Id::new(("tab", d.id)), egui::Sense::click());
                    if resp.clicked() {
                        self.active = i;
                    }
                    if resp.middle_clicked() {
                        close = Some(i);
                    }
                }
            });
        });
        if let Some(i) = close {
            self.close_tab(i, false);
        }
    }

    fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.18);
            ui.label(RichText::new("RISCode").size(46.0).color(theme::FG_BRIGHT).strong());
            ui.label(RichText::new("Editor y simulador RISC-V · compatible con RARS").size(15.0).color(theme::FG_DIM));
            ui.add_space(28.0);
            if ui.add_sized([220.0, 30.0], egui::Button::new("🗋  Archivo nuevo")).clicked() {
                self.cmd_new();
            }
            if ui.add_sized([220.0, 30.0], egui::Button::new("📄  Abrir archivo…")).clicked() {
                self.cmd_open_file();
            }
            if ui.add_sized([220.0, 30.0], egui::Button::new("📂  Abrir carpeta…")).clicked() {
                self.cmd_open_folder();
            }
            if !self.settings.recent.is_empty() {
                ui.add_space(20.0);
                ui.label(RichText::new("Recientes").color(theme::FG_DIM));
                for p in self.settings.recent.clone().into_iter().take(6) {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if ui.link(name).on_hover_text(p.display().to_string()).clicked() {
                        self.open_path(&p);
                    }
                }
            }
            ui.add_space(24.0);
            ui.label(RichText::new("F5 correr · F10 paso a paso · F9 punto de parada · Ctrl+B ensamblar").color(theme::FG_DIM).size(12.0));
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.pending else { return };
        let (title, names) = match p {
            Pending::CloseTab(i) => (
                "¿Guardar los cambios?",
                vec![self.docs.get(*i).map(|d| d.name()).unwrap_or_default()],
            ),
            Pending::Quit => ("Hay archivos sin guardar", self.docs.iter().filter(|d| d.dirty).map(|d| d.name()).collect()),
        };
        let mut choice = None;
        egui::Modal::new(egui::Id::new("pending")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new(title).strong().size(16.0));
            ui.add_space(6.0);
            for n in &names {
                ui.label(format!("• {n}"));
            }
            ui.label("Si no los guardas, se pierden los cambios.");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Guardar").clicked() {
                    choice = Some(0);
                }
                if ui.button("No guardar").clicked() {
                    choice = Some(1);
                }
                if ui.button("Cancelar").clicked() {
                    choice = Some(2);
                }
            });
        });
        let Some(c) = choice else { return };
        let pending = self.pending.take().unwrap();
        match (pending, c) {
            (_, 2) => {}
            (Pending::CloseTab(i), 0) => {
                if self.cmd_save(i, false) {
                    self.close_tab(i, true);
                }
            }
            (Pending::CloseTab(i), _) => self.close_tab(i, true),
            (Pending::Quit, 0) => {
                let all_saved = (0..self.docs.len()).filter(|&i| self.docs[i].dirty).collect::<Vec<_>>().into_iter().all(|i| self.cmd_save(i, false));
                if all_saved {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            (Pending::Quit, _) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Opciones de prueba de la línea de comandos: teclas programadas y captura de pantalla.
    fn automation(&mut self, ctx: &egui::Context) {
        let ms = self.started.elapsed().as_millis() as u64;
        if self.startup.run {
            self.startup.run = false;
            self.cmd_run(ctx);
        }
        if let Some(p) = self.startup.panel.take() {
            self.settings.show_bottom = true;
            self.bottom_tab = match p.as_str() {
                "problemas" => BottomTab::Problems,
                "memoria" => BottomTab::Memory,
                "codigo" | "código" => BottomTab::MachineCode,
                _ => BottomTab::Terminal,
            };
        }
        while self.startup.step > 0 {
            self.startup.step -= 1;
            self.cmd_step();
        }
        while let Some(&(t, k)) = self.startup.keys.first() {
            if ms < t {
                break;
            }
            if let Some(s) = &mut self.session {
                s.machine.key_press(k);
            }
            self.startup.keys.remove(0);
        }
        if let Some((path, at)) = self.startup.screenshot.clone() {
            if ms >= at && !self.screenshot_requested {
                self.screenshot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let rgb: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]).collect();
                let _ = std::fs::write(&path, crate::png::rgb(img.size[0] as u32, img.size[1] as u32, &rgb));
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = self.last_frame.elapsed().as_secs_f32().min(0.25);
        self.last_frame = Instant::now();

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.docs.iter().any(|d| d.dirty) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
        self.automation(&ctx);
        self.shortcuts(&ctx);
        self.route_keys_to_program(&ctx);
        self.tick_session(&ctx, dt);

        // seguir la línea en ejecución
        let exec = self.exec_line();
        if let (Some((line, _)), Some(s)) = (exec, &self.session) {
            if self.last_exec_line != Some(line) {
                self.last_exec_line = Some(line);
                if let Some(d) = self.docs.iter_mut().find(|d| d.id == s.doc_id) {
                    if line < d.visible.0 || line > d.visible.1 {
                        d.goto_line = Some((line, false));
                    }
                }
            }
        }

        // título de la ventana, como VS Code: "archivo — RISCode"
        let title = match self.docs.get(self.active) {
            Some(d) => format!("{}{} — RISCode", if d.dirty { "● " } else { "" }, d.name()),
            None => "RISCode".to_string(),
        };
        if self.last_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        // ── barra superior ──
        egui::Panel::top("top").frame(egui::Frame::new().fill(theme::TITLE_BG).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
            self.top_bar(ui);
        });
        // ── barra de estado ──
        let status_color = match self.session.as_ref().map(|s| &s.state) {
            Some(RunState::Running | RunState::Paused | RunState::WaitingInput(_)) => theme::STATUS_RUNNING,
            Some(RunState::Error(_)) => theme::STATUS_ERROR,
            Some(RunState::Finished(c)) if *c >= 0 => theme::STATUS_DONE,
            None if self.docs.get(self.active).is_some_and(|d| !d.errors.is_empty()) => theme::STATUS_ERROR,
            _ => theme::STATUS_IDLE,
        };
        egui::Panel::bottom("status").exact_size(24.0).frame(egui::Frame::new().fill(status_color).inner_margin(egui::Margin::symmetric(10, 0))).show(ui, |ui| {
            self.status_bar(ui);
        });
        // ── barra de actividades ──
        egui::Panel::left("activity").exact_size(48.0).resizable(false).frame(egui::Frame::new().fill(theme::ACTIVITY_BG)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            self.side_view_button(ui, SideView::Explorer, "🗐", "Explorador");
            self.side_view_button(ui, SideView::Reference, "📖", "Referencia RISC-V");
        });
        if self.settings.show_side {
            egui::Panel::left("side").default_size(250.0).min_size(160.0).resizable(true).frame(egui::Frame::new().fill(theme::SIDEBAR_BG).inner_margin(egui::Margin::symmetric(10, 6))).show(ui, |ui| {
                match self.side {
                    SideView::Explorer => self.panel_explorer(ui),
                    SideView::Reference => self.panel_reference(ui),
                }
            });
        }
        if self.settings.show_right {
            egui::Panel::right("right").default_size(400.0).min_size(260.0).resizable(true).frame(egui::Frame::new().fill(theme::SIDEBAR_BG).inner_margin(egui::Margin::symmetric(10, 6))).show(ui, |ui| {
                self.panel_display(ui);
                ui.separator();
                self.panel_registers(ui);
            });
        }
        if self.settings.show_bottom {
            egui::Panel::bottom("bottom").default_size(210.0).min_size(80.0).resizable(true).frame(egui::Frame::new().fill(theme::EDITOR_BG).inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 4 }).stroke(egui::Stroke::new(1.0, theme::BORDER))).show(ui, |ui| {
                self.bottom_tabs(ui);
                match self.bottom_tab {
                    BottomTab::Terminal => self.panel_terminal(ui),
                    BottomTab::Problems => self.panel_problems(ui),
                    BottomTab::Memory => self.panel_memory(ui),
                    BottomTab::MachineCode => self.panel_machine_code(ui),
                }
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::EDITOR_BG)).show(ui, |ui| {
            if self.docs.is_empty() {
                self.welcome(ui);
                return;
            }
            egui::Frame::new().fill(theme::SIDEBAR_BG).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.tabs(ui);
            });
            let session_doc = self.session.as_ref().map(|s| s.doc_id);
            let doc = &mut self.docs[self.active];
            let is_session_doc = session_doc == Some(doc.id);
            let s = self.session.as_ref().filter(|_| is_session_doc);
            let ectx = EditorContext {
                exec_line: if is_session_doc { exec } else { None },
                regs: s.map(|s| &s.machine.regs),
                labels: s.map(|s| &s.prog.labels),
                locked: false,
            };
            let cache = self.caches.entry(doc.id).or_default();
            editor::show(ui, doc, cache, &ectx);
        });

        self.dialogs(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.settings.open_files = self.docs.iter().filter_map(|d| d.path.clone()).collect();
        self.settings.active = self.active;
        self.settings.workspace = self.explorer.root.clone();
        eframe::set_value(storage, "riscode", &self.settings);
    }
}
