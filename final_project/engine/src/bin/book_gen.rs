use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let depth: u8 = args
        .next()
        .unwrap_or_else(|| "8".into())
        .parse()
        .expect("usage: book_gen [depth] [out_path]");
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| format!("engine/book_d{depth}.bin"))
        .into();
    println!("generating depth-{depth} book → {}", out.display());
    let start = std::time::Instant::now();
    engine::book::generate(&out, depth).expect("book generation failed");
    println!("done in {:?}", start.elapsed());
}
