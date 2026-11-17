//! Corre un .asm sin ventana, con un reloj virtual (las pausas de la syscall 32 no esperan de
//! verdad), aprieta teclas en ciertos momentos y guarda fotos de la pantalla bitmap en PNG.
//!
//! Uso:
//!   cargo run --release -p riscode-sim --example correr -- juego.asm salida 500:1 3000:d 4000:foto
//!
//! Cada evento es `ms:tecla` (una letra o número) o `ms:foto` (guarda salida_<ms>.png).
//! La pantalla es la de RARS: unidades de 8x8, 512x512, base en gp (64x64 unidades).

use std::collections::HashSet;
use std::time::Instant;

use riscode_sim::{memory::GP_INIT, Machine, Stop};

const W: usize = 64;
const H: usize = 64;
const ESCALA: usize = 4;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).expect("no se pudo leer el .asm");
    let src = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let prefijo = &args[2];
    let mut eventos: Vec<(u64, String)> = args[3..]
        .iter()
        .map(|e| {
            let (t, k) = e.split_once(':').expect("evento = ms:tecla");
            (t.parse().unwrap(), k.to_string())
        })
        .collect();
    eventos.sort_by_key(|e| e.0);
    let fin = eventos.last().map(|e| e.0).unwrap_or(1000);

    let prog = match riscode_asm::assemble(&src) {
        Ok(p) => p,
        Err(e) => {
            for x in e.iter().take(10) {
                eprintln!("línea {}: {}", x.line + 1, x.msg);
            }
            std::process::exit(1);
        }
    };
    let mut m = Machine::new(&prog);
    let bps = HashSet::new();
    let mut reloj: u64 = 0; // ms virtuales
    let mut dormir_hasta: u64 = 0;
    let mut sig = 0;
    let real = Instant::now();
    while reloj <= fin {
        while sig < eventos.len() && eventos[sig].0 <= reloj {
            let (t, k) = &eventos[sig];
            if k == "foto" {
                let nombre = format!("{prefijo}_{t}.png");
                guardar_png(&m, &nombre);
                println!("[{t} ms] foto -> {nombre}");
            } else {
                let c = k.as_bytes()[0];
                m.key_press(c);
                println!("[{t} ms] tecla '{}'", c as char);
            }
            sig += 1;
        }
        if reloj < dormir_hasta {
            reloj = dormir_hasta.min(eventos.get(sig).map(|e| e.0).unwrap_or(u64::MAX));
            continue;
        }
        m.now_ms = 1_700_000_000_000 + reloj;
        let (_, stop) = m.run(200_000, &bps);
        match stop {
            Stop::Budget => reloj += 1,
            Stop::Sleep(ms) => dormir_hasta = reloj + ms as u64,
            Stop::Exit(c) => {
                println!("[{reloj} ms] el programa terminó con código {c}");
                break;
            }
            Stop::Error(e) => {
                println!("[{reloj} ms] ERROR: {e}");
                break;
            }
            other => {
                println!("[{reloj} ms] se detuvo: {other:?}");
                break;
            }
        }
    }
    let dt = real.elapsed().as_secs_f64();
    println!(
        "{} instrucciones en {:.2} s reales ({:.1} millones/s). Consola: {:?}",
        m.instr_count,
        dt,
        m.instr_count as f64 / dt / 1e6,
        m.output.chars().take(200).collect::<String>()
    );
}

fn guardar_png(m: &Machine, nombre: &str) {
    let px = m.mem.slice(GP_INIT, (W * H * 4) as u32).unwrap();
    let (w, h) = (W * ESCALA, H * ESCALA);
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for y in 0..h {
        raw.push(0); // filtro "ninguno"
        for x in 0..w {
            let i = ((y / ESCALA) * W + x / ESCALA) * 4;
            raw.extend_from_slice(&[px[i + 2], px[i + 1], px[i]]);
        }
    }
    std::fs::write(nombre, png_rgb(w as u32, h as u32, &raw)).unwrap();
}

/// PNG mínimo sin compresión (bloques "stored" de deflate).
fn png_rgb(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i == blocks.len() - 1) as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
