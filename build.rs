use std::path::PathBuf;

fn main() {
    let dir: PathBuf = ["tree-sitter-typescript", "tsx", "src"].iter().collect();

    cc::Build::new()
        .include(&dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs")
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.c"))
        .compile("tree-sitter-typescript");
}
