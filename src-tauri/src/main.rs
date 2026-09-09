// Entrada do binário: apenas delega ao `run()` do crate lib (lib.rs),
// onde o aplicativo Tauri é de fato montado.
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    scd_lib::run()
}
