//! Compara el código máquina de RISCode con el "Dump" de RARS 1.6 (generado con
//! `java -jar rars.jar a dump .text HexText ... dump .data HexText ... archivo.asm`).

use riscode_asm::{assemble, exportar_palabras, palabras_de_datos, Formato};

#[test]
fn todas_las_instrucciones_igual_que_rars() {
    let src = include_str!("golden/todas_las_instrucciones.asm");
    let prog = assemble(src).unwrap_or_else(|e| panic!("{e:#?}"));
    let text = exportar_palabras(prog.text.iter().copied(), Formato::Hex);
    let data = exportar_palabras(palabras_de_datos(&prog.data), Formato::Hex);
    let rars_text = include_str!("golden/todas_las_instrucciones.text.hex");
    let rars_data = include_str!("golden/todas_las_instrucciones.data.hex");
    for (i, (a, b)) in text.lines().zip(rars_text.lines()).enumerate() {
        assert_eq!(a, b, "instrucción {i} (dirección {:#010x}) distinta a RARS", 0x0040_0000 + 4 * i);
    }
    assert_eq!(text.lines().count(), rars_text.lines().count());
    assert_eq!(data, rars_data);
}
