fn main() {
    // The menu bar icon's template images are read at compile time (`tray.rs`, include_image!).
    println!("cargo:rerun-if-changed=icons/original/png/menubar-idle-template-2x.png");
    println!("cargo:rerun-if-changed=icons/original/png/menubar-waiting-template-2x.png");
    tauri_build::build()
}
