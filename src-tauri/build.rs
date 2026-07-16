fn main() {
    println!("cargo:rerun-if-env-changed=DOGU_UPDATER_PUBLIC_KEY");
    tauri_build::build()
}
