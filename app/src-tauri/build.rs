fn main() {
    // The menu bar icon's template images are read at compile time (`tray.rs`, include_image!).
    println!("cargo:rerun-if-changed=icons/original/png/menubar-idle-template-2x.png");
    println!("cargo:rerun-if-changed=icons/original/png/menubar-waiting-template-2x.png");
    // The status line's reader (0113) uses none of the app's frameworks: drop the ones it doesn't
    // reference, so dyld never loads AppKit or WebKit on a status-line render.
    if std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() == Ok("apple") {
        println!("cargo:rustc-link-arg-bin=muster-statusline=-Wl,-dead_strip_dylibs");
    }
    tauri_build::build()
}
