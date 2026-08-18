// Concern: bundling standards/*.md into the binary — the folder is the list | Non-concern: what any standard says, or which gate declares one | IO: (standards/*.md) -> OUT_DIR/standards.rs

use std::fmt::Write as _;
use std::path::Path;

// The folder is the source of truth. Enumerated in Rust instead, a standard would exist twice — as a file and as a line someone has to remember to add — and the one that ships would be whichever of the two was edited last.
fn main() {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir");
    let dir = Path::new(&root).join("standards");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut found: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let name = path
                .file_stem()
                .expect("a .md file has a stem")
                .to_string_lossy()
                .into_owned();
            (name, path.to_string_lossy().into_owned())
        })
        .collect();
    assert!(!found.is_empty(), "no standards in {}", dir.display());
    // Sorted, because a directory hands them back in whatever order the filesystem likes, and a build that reorders what `--standards` prints from one machine to the next is one nobody can diff.
    found.sort();

    let mut out = format!("pub const SHIPPED: [(&str, &str); {}] = [\n", found.len());
    for (name, path) in &found {
        // Rebuilt when any one of them changes, not only when the folder's own entries do.
        println!("cargo:rerun-if-changed={path}");
        let _ = writeln!(out, "    ({name:?}, include_str!({path:?})),");
    }
    out.push_str("];\n");

    let generated =
        Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("standards.rs");
    std::fs::write(&generated, out).expect("write the generated list");
}
