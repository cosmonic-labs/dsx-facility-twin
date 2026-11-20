//! Embed ui/dist into the component: GET /index.html, /assets/*.js, … are served from memory.
use std::fmt::Write;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn ctype(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "json" => "application/json",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files = vec![];
    walk(&dist, &mut files);
    files.sort();
    let mut s = String::from("pub fn lookup(p: &str) -> Option<(&'static str, &'static [u8])> {\n    match p {\n");
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let rel = f.strip_prefix(&dist).unwrap().to_string_lossy().replace('\\', "/");
        let abs = f.canonicalize().unwrap();
        writeln!(s, "        {:?} => Some(({:?}, include_bytes!({:?}))),", format!("/{rel}"), ctype(f), abs.display().to_string()).unwrap();
    }
    s.push_str("        _ => None,\n    }\n}\n");
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("assets.rs"), s).unwrap();
}
