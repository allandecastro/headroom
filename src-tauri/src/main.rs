// Prevent a console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    headroom_lib::run()
}
