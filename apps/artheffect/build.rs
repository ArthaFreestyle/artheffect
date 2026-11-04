//! Windows only: embed the app icon and version info (VERSIONINFO) into `artheffect.exe`.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `ARTHEFFECT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/artheffect.ico");
    println!("cargo:rerun-if-env-changed=ARTHEFFECT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/artheffect.ico")
        .set("ProductName", "Artheffect")
        .set("FileDescription", "Artheffect motion graphics and visual effects")
        .set("LegalCopyright", "Copyright (c) 2026 The EffectCraft contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "artheffect.exe")
        .set("InternalName", "artheffect");
    if let Err(e) = res.compile() {
        if std::env::var_os("ARTHEFFECT_REQUIRE_WINRES").is_some() {
            panic!("embedding Windows resources failed: {e}");
        }
        println!("cargo:warning=artheffect.exe built without icon/version resources: {e}");
    }
}
