//! Documentos abiertos (pestañas) y el explorador de archivos.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use riscode_asm::AsmError;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub text: String,
    pub dirty: bool,
    /// Sube cada vez que cambia el texto (para saber si el programa ensamblado está viejo).
    pub version: u64,
    pub breakpoints: BTreeSet<usize>,
    pub errors: Vec<AsmError>,
    /// (línea, mover el cursor ahí)
    pub goto_line: Option<(usize, bool)>,
    /// Líneas visibles en el último cuadro (primera, última).
    pub visible: (usize, usize),
    pub cursor: (usize, usize),
    /// El archivo no era UTF-8 (se leyó como Latin-1 y se guardará como UTF-8).
    pub was_latin1: bool,
}

impl Document {
    pub fn new_empty() -> Self {
        Document {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            path: None,
            text: String::new(),
            dirty: false,
            version: 0,
            breakpoints: BTreeSet::new(),
            errors: Vec::new(),
            goto_line: None,
            visible: (0, 0),
            cursor: (0, 0),
            was_latin1: false,
        }
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let (text, was_latin1) = match String::from_utf8(bytes) {
            Ok(s) => (s, false),
            // Los .asm hechos en Windows a veces vienen en Latin-1 (tildes, ñ).
            Err(e) => (e.into_bytes().iter().map(|&b| b as char).collect(), true),
        };
        let mut d = Document::new_empty();
        d.text = text.replace("\r\n", "\n");
        d.path = Some(path.to_path_buf());
        d.was_latin1 = was_latin1;
        Ok(d)
    }

    pub fn name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sin título.asm".into())
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(p) = &self.path else {
            return Err(std::io::Error::other("el archivo no tiene nombre"));
        };
        std::fs::write(p, &self.text)?;
        self.dirty = false;
        self.was_latin1 = false;
        Ok(())
    }

    pub fn mark_changed(&mut self) {
        self.dirty = true;
        self.version += 1;
    }
}

pub struct Explorer {
    pub root: Option<PathBuf>,
    pub expanded: HashSet<PathBuf>,
    /// Carpetas ya leídas (se vuelven a leer cada 2 segundos).
    pub cache: HashMap<PathBuf, (Instant, Vec<Entry>)>,
}

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

impl Explorer {
    pub fn list_cached(&mut self, dir: &Path) -> Vec<Entry> {
        if let Some((t, v)) = self.cache.get(dir) {
            if t.elapsed() < Duration::from_secs(2) {
                return v.clone();
            }
        }
        let v = Self::list(dir);
        self.cache.insert(dir.to_path_buf(), (Instant::now(), v.clone()));
        v
    }

    pub fn list(dir: &Path) -> Vec<Entry> {
        let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
        let mut v: Vec<Entry> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name == "target" {
                    return None;
                }
                let is_dir = e.file_type().ok()?.is_dir();
                Some(Entry { path: e.path(), name, is_dir })
            })
            .collect();
        v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        v
    }
}

pub fn is_asm(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("asm" | "s" | "rars")
    )
}
