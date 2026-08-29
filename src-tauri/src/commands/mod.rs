//! Fronteira entre o frontend e o núcleo: tudo que o `invoke()` alcança está aqui.
//!
//! Regra da casa: comando não tem lógica de negócio. Ele valida a entrada, chama
//! `core`/`storage` e traduz o erro para `String` (que o frontend embrulha em
//! `TauriCommandError`). Os wrappers tipados do outro lado ficam em `src/lib/tauri/`.

pub mod automation;
pub mod casa;
pub mod conhecimento;
pub mod chat;
pub mod navegador;
pub mod settings;
pub mod system;
pub mod voice;
