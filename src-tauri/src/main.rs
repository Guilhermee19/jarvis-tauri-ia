// Em release, não abre console do Windows junto com a janela.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    jarvis_lib::run()
}
