#![allow(clippy::print_stderr)]

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bytes = std::fs::read(path).unwrap();
    let pex = papyrus_pex::PexFile::from_bytes(&bytes).unwrap();
    for (i, s) in pex.string_table.iter().enumerate() {
        eprint!("{i}:{s:?} ");
    }
    eprintln!();
    for f in &pex.debug_info.as_ref().unwrap().functions {
        eprintln!(
            "obj={} state={}({:?}) fn={} ty={:?} n={}",
            f.object_name,
            f.state_name,
            pex.str(f.state_name),
            pex.str(f.function_name),
            f.function_type,
            f.instruction_line_numbers.len()
        );
    }
    for s in &pex.objects[0].states {
        eprintln!("state id {} {:?}", s.name, pex.str(s.name));
    }
    let rt = pex.to_bytes().unwrap();
    eprintln!("roundtrip equal: {}", rt == bytes);
}
