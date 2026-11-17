//! Ensambla un .asm y escribe el código máquina en hexadecimal (como el Dump de RARS).
//! Uso: cargo run -p riscode-asm --example dump -- archivo.asm salida_text.hex salida_data.hex
use riscode_asm::{assemble, exportar_palabras, palabras_de_datos, Formato};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).expect("no se pudo leer el archivo");
    let src = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    match assemble(&src) {
        Ok(p) => {
            std::fs::write(&args[2], exportar_palabras(p.text.iter().copied(), Formato::Hex)).unwrap();
            std::fs::write(&args[3], exportar_palabras(palabras_de_datos(&p.data), Formato::Hex)).unwrap();
            println!("{} instrucciones, {} bytes de datos", p.text.len(), p.data.len());
        }
        Err(errs) => {
            for e in errs.iter().take(20) {
                println!("línea {}: {}", e.line + 1, e.msg);
            }
            std::process::exit(1);
        }
    }
}
