// Evita la consola extra en Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    miel_lib::run();
}
