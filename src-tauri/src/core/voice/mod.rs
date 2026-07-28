//! PLACEHOLDER — voz (v0.3).
//!
//! Três responsabilidades, provavelmente um arquivo cada:
//! - `wake_word.rs`: escuta contínua do microfone e detecção da palavra-gatilho.
//! - `stt.rs`: transcrição da fala em texto, alimentando o mesmo fluxo de
//!   `commands::chat::send_message` que o campo de texto usa hoje.
//! - `tts.rs`: síntese da resposta em áudio.
//!
//! Roda em uma task de background iniciada no `setup` de `lib.rs`, e conversa com a
//! UI por eventos (`jarvis://wake-word`, `jarvis://transcript`) em vez de `invoke` —
//! o frontend já escuta eventos assim em `src/lib/tauri/events.ts`.
