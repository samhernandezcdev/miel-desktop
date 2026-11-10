fn main() {
    // Los comandos propios deben declararse para poder exigir permisos explícitos (mínimo privilegio).
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "get_view",
                "get_display_prefs",
                "report_frontend_error",
                "get_settings",
                "set_setting",
            ])),
    )
    .expect("fallo en tauri-build");

    // Solo en los tests de INTEGRACIÓN (Windows/MSVC): incrusta Common-Controls v6, que el runtime
    // de pruebas de Tauri (muda) necesita para cargar. No afecta a la app ni a los tests unitarios.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}/test.manifest", dir.replace(char::from(92), "/"));
        println!("cargo:rerun-if-changed=test.manifest");
    }
}
