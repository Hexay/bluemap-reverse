// vendor/ is xpple/cubiomes @ 18edd56 (MIT, vendor/LICENSE) minus tests.c and loot/, with `bmr patch` edits for
// MSVC (no VLAs); see shim/msvc_compat.h.
fn main() {
    let mut build = cc::Build::new();
    build.include("vendor").file("shim/shim.c").warnings(false).opt_level(3);
    for dir in ["vendor", "vendor/features"] {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.path().extension().is_some_and(|e| e == "c") {
                build.file(entry.path());
            }
        }
    }
    if build.get_compiler().is_like_msvc() {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        build.flag(format!("/FI{dir}/shim/msvc_compat.h"));
    }
    build.compile("cubiomes");
    println!("cargo:rerun-if-changed=vendor");
    println!("cargo:rerun-if-changed=shim");
}
