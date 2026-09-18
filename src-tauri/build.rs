fn main() {
    // Force a rebuild when the frontend changes: tauri::generate_context!()
    // embeds ../dist at compile time, but cargo has no other way to know its
    // content moved (stale exe otherwise ships an old build of the UI).
    println!("cargo:rerun-if-changed=../dist");
    tauri_build::build()
}
