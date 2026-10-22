//! `rust-embed` needs `apps/web/dist` to exist at compile time. A fresh clone
//! has not run `bun run build` yet, so create the folder (empty) instead of
//! failing; the server then answers non-API paths with a 503 naming the build command.
fn main() {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/dist");
    std::fs::create_dir_all(&dist).expect("create apps/web/dist");
    println!("cargo:rerun-if-changed=../../apps/web/dist");
}
