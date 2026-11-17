//! RISCode: editor de código estilo VS Code para ensamblador RISC-V, con simulador,
//! pantalla bitmap y teclado como RARS.

mod app;
mod editor;
mod files;
mod panels;
mod png;
mod session;
mod theme;

use std::path::PathBuf;

const AYUDA: &str = "RISCode — editor y simulador RISC-V

Uso: riscode [archivos.asm…] [opciones]

  --correr               Ensambla y corre el primer archivo al abrir
  --paso N               Empieza paso a paso y avanza N instrucciones
  --panel NOMBRE         Abre el panel inferior: terminal, problemas, memoria o codigo
  --ayuda                Muestra esta ayuda

Opciones para pruebas automáticas:
  --teclas 2000:1,6500:d Aprieta teclas a esos milisegundos de haber abierto
  --captura f.png:8000   Guarda una captura de la ventana a los 8000 ms y cierra";

fn main() -> eframe::Result {
    let mut startup = app::Startup::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--ayuda" | "--help" | "-h" => {
                println!("{AYUDA}");
                return Ok(());
            }
            "--correr" => startup.run = true,
            "--panel" => startup.panel = args.next(),
            "--paso" => startup.step = args.next().and_then(|n| n.parse().ok()).unwrap_or(0) + 1,
            "--teclas" => {
                for item in args.next().unwrap_or_default().split(',') {
                    if let Some((t, k)) = item.split_once(':') {
                        if let (Ok(t), Some(c)) = (t.parse(), k.bytes().next()) {
                            startup.keys.push((t, c));
                        }
                    }
                }
                startup.keys.sort();
            }
            "--captura" => {
                let v = args.next().unwrap_or_default();
                let (p, t) = v.rsplit_once(':').unwrap_or((&v, "3000"));
                startup.screenshot = Some((PathBuf::from(p), t.parse().unwrap_or(3000)));
            }
            _ => startup.files.push(std::fs::canonicalize(&a).unwrap_or_else(|_| PathBuf::from(&a))),
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RISCode")
            .with_app_id("riscode")
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([820.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("RISCode", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, startup)))))
}
