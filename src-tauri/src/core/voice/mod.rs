//! Voz: microfone (entrada) e fala sintetizada (saída).
//!
//! As duas pontas são independentes e ficam expostas como comandos separados. A
//! v0.2+ pluga o agente no meio delas sem mudar nenhuma assinatura: a transcrição
//! lê o WAV que [`VoiceState::stop_recording`] deixa em disco, e a resposta do
//! modelo entra em [`TtsEngine::synthesize`] no lugar da frase de teste.
//!
//! O que ainda NÃO existe aqui: `wake_word.rs`.
//!
//! Sobre o formato do áudio: a gravação entrega WAV mono PCM de 16 bits, que é o que
//! o Whisper quer — MENOS na taxa de amostragem. O microfone abre no formato nativo
//! do dispositivo (tipicamente 48 kHz) e o whisper.cpp recusa qualquer coisa que não
//! seja 16 kHz. Quem resolve isso é `stt.rs`, na leitura, sem mexer no `mic.rs`.

mod mic;
mod stt;
mod tts;

use std::path::Path;
use std::sync::Mutex;

pub use mic::{Recorder, Recording};
pub use stt::transcribe;
pub use tts::{play, ElevenLabs, TtsEngine, Voice};

use crate::core::lock;

#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    #[error("nenhum microfone encontrado — conecte um dispositivo de entrada e tente de novo")]
    NoInputDevice,
    #[error(
        "o sistema negou acesso ao microfone. No Windows: Configurações › Privacidade e segurança › Microfone"
    )]
    MicrophoneDenied,
    #[error("falha no microfone: {0}")]
    Microphone(String),
    #[error("já existe uma gravação em andamento")]
    AlreadyRecording,
    #[error("nenhuma gravação em andamento")]
    NotRecording,
    #[error("falha ao gravar o arquivo WAV: {0}")]
    WavWrite(String),
    #[error("falha ao ler o áudio gravado: {0}")]
    WavLeitura(String),
    #[error("gravação curta demais — segure o botão enquanto fala")]
    GravacaoCurta,
    #[error("não ouvi nada — fale mais perto do microfone")]
    NadaOuvido,
    #[error(
        "o serviço de transcrição não respondeu em {0}. Confira o caminho do Whisper em Configurações"
    )]
    TranscricaoOffline(String),
    #[error("a transcrição falhou (HTTP {status}): {corpo}")]
    TranscricaoRecusada { status: u16, corpo: String },
    #[error("falha de rede ao transcrever: {0}")]
    TranscricaoRede(String),
    #[error("defina a API key da ElevenLabs em Configurações para usar a voz")]
    MissingApiKey,
    #[error("nenhuma voz disponível na conta da ElevenLabs")]
    NoVoiceAvailable,
    #[error("a ElevenLabs recusou a chamada (HTTP {status}): {body}")]
    TtsRejected { status: u16, body: String },
    #[error("falha de rede ao falar com a ElevenLabs: {0}")]
    TtsNetwork(String),
    #[error("falha ao tocar o áudio: {0}")]
    Playback(String),
}

/// Dono do que é caro ou único: a gravação em andamento e o pool de conexões HTTP.
/// Registrado no Tauri como estado gerenciado, ao lado do `AppState`.
pub struct VoiceState {
    recorder: Mutex<Option<Recorder>>,
    http: reqwest::Client,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            recorder: Mutex::new(None),
            http: reqwest::Client::new(),
        }
    }

    pub fn is_recording(&self) -> bool {
        lock(&self.recorder).is_some()
    }

    /// O mesmo pool que fala com a ElevenLabs serve para falar com o Whisper local —
    /// clonar compartilha as conexões.
    pub fn http(&self) -> reqwest::Client {
        self.http.clone()
    }

    pub fn start_recording<F>(&self, on_level: F) -> Result<(), VoiceError>
    where
        F: Fn(f32) + Send + 'static,
    {
        let mut slot = lock(&self.recorder);
        if slot.is_some() {
            return Err(VoiceError::AlreadyRecording);
        }

        *slot = Some(Recorder::start(on_level)?);
        Ok(())
    }

    pub fn stop_recording(&self, path: &Path) -> Result<Recording, VoiceError> {
        let recorder = lock(&self.recorder)
            .take()
            .ok_or(VoiceError::NotRecording)?;
        recorder.stop(path)
    }

    /// Motor de TTS com a key que está nas configurações AGORA — o usuário pode
    /// colar a key e testar a voz sem reiniciar o app.
    pub fn tts(&self, api_key: &str) -> Result<Box<dyn TtsEngine>, VoiceError> {
        if api_key.trim().is_empty() {
            return Err(VoiceError::MissingApiKey);
        }

        Ok(Box::new(ElevenLabs::new(
            self.http.clone(),
            api_key.to_owned(),
        )))
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        Self::new()
    }
}
