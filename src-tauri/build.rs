fn main() {
    // Flavor is baked in via option_env! — recompile when it changes.
    println!("cargo:rerun-if-env-changed=BIBLE_APP_TIER");
    println!("cargo:rerun-if-env-changed=BIBLE_APP_MODELS");
    tauri_build::build()
}
