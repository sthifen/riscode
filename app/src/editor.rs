//! El editor de código: números de línea, puntos de parada, colores, errores subrayados,
//! línea en ejecución y ayudas al pasar el mouse.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use egui::text::{ByteIndex, CCursor, CCursorRange, LayoutJob};
use egui::{Galley, Pos2, Rect, Sense, Shape, Stroke, TextFormat, Ui, Vec2};
use riscode_asm::highlight::{highlight_line, scan_symbols};
use riscode_asm::lexer::{is_ident_char, reg_num, ABI_NAMES};
use riscode_asm::AsmError;

use crate::files::Document;
use crate::theme;

/// Guarda el último texto coloreado para no recalcularlo en cada cuadro.
#[derive(Default)]
pub struct HighlightCache {
    key: u64,
    galley: Option<Arc<Galley>>,
}

impl HighlightCache {
    fn galley(&mut self, ui: &Ui, text: &str, errors: &[AsmError]) -> Arc<Galley> {
        let mut h = DefaultHasher::new();
        text.hash(&mut h);
        for e in errors {
            (e.line, e.start, e.end).hash(&mut h);
        }
        let key = h.finish();
        if let Some(g) = &self.galley {
            if self.key == key {
                return g.clone();
            }
        }
        let job = build_job(text, errors);
        let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
        self.key = key;
        self.galley = Some(g.clone());
        g
    }
}

fn build_job(text: &str, errors: &[AsmError]) -> LayoutJob {
    let syms = scan_symbols(text);
    let mut by_line: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for e in errors {
        by_line.entry(e.line).or_default().push((e.start, e.end.max(e.start + 1)));
    }
    let font = theme::mono();
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        let errs = by_line.get(&i);
        for (range, class) in highlight_line(line, &syms) {
            let underline = errs
                .is_some_and(|v| v.iter().any(|(s, e)| range.start < *e && range.end > *s))
                .then(|| Stroke::new(1.5, theme::ERROR))
                .unwrap_or(Stroke::NONE);
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: ByteIndex(offset + range.start)..ByteIndex(offset + range.end),
                format: TextFormat { font_id: font.clone(), color: theme::syntax(class), underline, ..Default::default() },
            });
        }
        offset += line.len();
        if offset < text.len() {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: ByteIndex(offset)..ByteIndex(offset + 1),
                format: TextFormat { font_id: font.clone(), ..Default::default() },
            });
            offset += 1;
        }
    }
    job.text = text.to_string();
    job
}

pub struct EditorContext<'a> {
    /// Línea en ejecución y si es un error.
    pub exec_line: Option<(usize, bool)>,
    pub regs: Option<&'a [u32; 32]>,
    pub labels: Option<&'a HashMap<String, u32>>,
    pub locked: bool,
}

pub fn line_start_char(text: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut chars = 0;
    let mut l = 0;
    for c in text.chars() {
        chars += 1;
        if c == '\n' {
            l += 1;
            if l == line {
                return chars;
            }
        }
    }
    chars
}

fn char_to_line_col(text: &str, idx: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, c) in text.chars().enumerate() {
        if i == idx {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub fn show(ui: &mut Ui, doc: &mut Document, cache: &mut HighlightCache, ectx: &EditorContext) {
    let ctx = ui.ctx().clone();
    let font = theme::mono();
    let row_h = ctx.fonts_mut(|f| f.row_height(&font));
    let char_w = ctx.fonts_mut(|f| f.glyph_width(&font, '0'));
    let n_lines = doc.text.matches('\n').count() + 1;
    let digits = n_lines.to_string().len().max(3);
    let gutter_w = digits as f32 * char_w + 36.0;
    let edit_id = egui::Id::new(("editor", doc.id));
    let margin = Vec2::new(8.0, 4.0);

    let mut scroll = egui::ScrollArea::both().id_salt(("scroll", doc.id)).auto_shrink([false, false]);
    if let Some((line, move_cursor)) = doc.goto_line.take() {
        let view_h = ui.available_height();
        let y = (line as f32 * row_h - view_h / 3.0).max(0.0);
        scroll = scroll.vertical_scroll_offset(y);
        if move_cursor {
            let idx = line_start_char(&doc.text, line);
            let mut st = egui::TextEdit::load_state(&ctx, edit_id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
            st.store(&ctx, edit_id);
            ctx.memory_mut(|m| m.request_focus(edit_id));
        }
    }

    let errors = doc.errors.clone();
    let error_lines: HashMap<usize, &AsmError> = errors.iter().map(|e| (e.line, e)).collect();

    scroll.show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let gutter_h = (n_lines as f32 * row_h + 2.0 * margin.y).max(ui.available_height());
            let (gutter_rect, gutter_resp) =
                ui.allocate_exact_size(Vec2::new(gutter_w, gutter_h), Sense::click());
            let bg_idx = ui.painter().add(Shape::Noop);

            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| cache.galley(ui, buf.as_str(), &errors);
            let output = egui::TextEdit::multiline(&mut doc.text)
                .id(edit_id)
                .code_editor()
                .font(font.clone())
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(margin.x as i8, margin.y as i8))
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .lock_focus(true)
                .interactive(!ectx.locked)
                .layouter(&mut layouter)
                .show(ui);
            let resp = output.response.response.clone();
            if resp.changed() {
                doc.mark_changed();
                doc.errors.clear();
            }
            let galley = &output.galley;
            let gpos = output.galley_pos;
            let row_y = |line: usize| -> f32 {
                galley.rows.get(line).map(|r| gpos.y + r.pos.y).unwrap_or(gpos.y + line as f32 * row_h)
            };

            // cursor
            let cursor_line = output.cursor_range.map(|r| {
                let (l, c) = char_to_line_col(&doc.text, r.primary.index.0);
                doc.cursor = (l, c);
                l
            });
            let has_focus = resp.has_focus();

            // fondo: línea actual, línea en ejecución, líneas con error
            let clip = ui.clip_rect();
            let full_w = |y: f32| Rect::from_min_max(Pos2::new(gutter_rect.right(), y), Pos2::new(clip.right().max(resp.rect.right()), y + row_h));
            let mut bg = Vec::new();
            if let (Some(l), true) = (cursor_line, has_focus) {
                bg.push(Shape::rect_filled(full_w(row_y(l)), 0.0, theme::CURRENT_LINE));
            }
            for &l in error_lines.keys() {
                bg.push(Shape::rect_filled(full_w(row_y(l)), 0.0, theme::ERROR_LINE));
            }
            if let Some((l, is_err)) = ectx.exec_line {
                let color = if is_err { theme::ERROR_LINE } else { theme::EXEC_LINE };
                bg.push(Shape::rect_filled(full_w(row_y(l)), 0.0, color));
            }
            ui.painter().set(bg_idx, Shape::Vec(bg));

            // margen izquierdo: números de línea y puntos de parada
            let painter = ui.painter();
            painter.rect_filled(gutter_rect, 0.0, theme::EDITOR_BG);
            let first = (((clip.top() - gpos.y) / row_h).floor().max(0.0)) as usize;
            let last = ((((clip.bottom() - gpos.y) / row_h).ceil()) as usize + 1).min(n_lines);
            doc.visible = (first, last.saturating_sub(2).max(first));
            let hover_line = gutter_resp
                .hover_pos()
                .map(|p| ((p.y - gpos.y) / row_h).floor().max(0.0) as usize)
                .filter(|l| *l < n_lines);
            let dot_x = gutter_rect.left() + 11.0;
            for line in first..last {
                let y = row_y(line);
                let is_cur = cursor_line == Some(line);
                let color = if error_lines.contains_key(&line) {
                    theme::ERROR
                } else if is_cur {
                    theme::FG_BRIGHT
                } else {
                    theme::FG_DIM
                };
                painter.text(
                    Pos2::new(gutter_rect.right() - 12.0, y),
                    egui::Align2::RIGHT_TOP,
                    (line + 1).to_string(),
                    font.clone(),
                    color,
                );
                let center = Pos2::new(dot_x, y + row_h / 2.0);
                if doc.breakpoints.contains(&line) {
                    painter.circle_filled(center, 5.0, theme::BREAKPOINT);
                } else if hover_line == Some(line) {
                    painter.circle_filled(center, 5.0, theme::BREAKPOINT.gamma_multiply(0.4));
                }
                if ectx.exec_line.map(|e| e.0) == Some(line) {
                    let c = if ectx.exec_line.unwrap().1 { theme::ERROR } else { theme::EXEC_ARROW };
                    let x = dot_x + 2.0;
                    let m = y + row_h / 2.0;
                    painter.add(Shape::convex_polygon(
                        vec![Pos2::new(x - 7.0, m - 5.0), Pos2::new(x + 1.0, m - 5.0), Pos2::new(x + 6.0, m), Pos2::new(x + 1.0, m + 5.0), Pos2::new(x - 7.0, m + 5.0)],
                        c,
                        Stroke::NONE,
                    ));
                }
            }
            if gutter_resp.clicked() {
                if let Some(l) = hover_line {
                    if !doc.breakpoints.remove(&l) {
                        doc.breakpoints.insert(l);
                    }
                }
            }
            if hover_line.is_some() {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            // ayuda al pasar el mouse por encima de una palabra
            if let Some(p) = resp.hover_pos() {
                let line = ((p.y - gpos.y) / row_h).floor().max(0.0) as usize;
                if let Some(row) = galley.rows.get(line) {
                    let col = row.char_at(p.x - gpos.x - row.pos.x).0;
                    if let Some(text_line) = doc.text.split('\n').nth(line) {
                        hover_tooltip(&resp, text_line, line, col, &error_lines, ectx);
                    }
                }
            }
        });
    });
}

fn hover_tooltip(
    resp: &egui::Response,
    line: &str,
    line_no: usize,
    col: usize,
    errors: &HashMap<usize, &AsmError>,
    ectx: &EditorContext,
) {
    let chars: Vec<char> = line.chars().collect();
    let byte_col = chars.iter().take(col).map(|c| c.len_utf8()).sum::<usize>();
    if let Some(e) = errors.get(&line_no) {
        if byte_col >= e.start.saturating_sub(1) && byte_col <= e.end + 1 || e.start == e.end {
            let msg = e.msg.clone();
            resp.clone().on_hover_ui_at_pointer(|ui| {
                ui.colored_label(theme::ERROR, format!("✖ {msg}"));
            });
            return;
        }
    }
    if col >= chars.len() || !is_ident_char(chars[col]) {
        return;
    }
    let mut s = col;
    while s > 0 && is_ident_char(chars[s - 1]) {
        s -= 1;
    }
    let mut e = col;
    while e < chars.len() && is_ident_char(chars[e]) {
        e += 1;
    }
    let word: String = chars[s..e].iter().collect();
    // si está antes de un '#', es un comentario
    if chars[..s].contains(&'#') {
        return;
    }
    let lower = word.to_ascii_lowercase();
    if let Some(r) = reg_num(&word) {
        let desc = match r {
            0 => "siempre vale 0",
            1 => "dirección de retorno",
            2 => "puntero de pila",
            3 => "puntero global (aquí empieza la pantalla bitmap)",
            4 => "puntero de hilo",
            5..=7 | 28..=31 => "temporal",
            8 | 9 | 18..=27 => "guardado (se debe preservar entre funciones)",
            10..=11 => "argumento / valor de retorno",
            _ => "argumento",
        };
        resp.clone().on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(format!("{} (x{r})", ABI_NAMES[r as usize])).monospace().strong());
            ui.label(desc);
            if let Some(regs) = ectx.regs {
                let v = regs[r as usize];
                ui.label(egui::RichText::new(format!("= {v:#010x} ({})", v as i32)).monospace().color(theme::syntax(riscode_asm::highlight::Class::Number)));
            }
        });
        return;
    }
    if let Some((syn, desc)) = riscode_asm::instr::documentacion(&lower) {
        resp.clone().on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(syn).monospace().strong().color(theme::syntax(riscode_asm::highlight::Class::Instruction)));
            ui.label(desc);
        });
        return;
    }
    if let Some(addr) = ectx.labels.and_then(|l| l.get(&word)) {
        let addr = *addr;
        resp.clone().on_hover_ui_at_pointer(|ui| {
            ui.label(egui::RichText::new(format!("{word}: {addr:#010x}")).monospace());
        });
    }
}
