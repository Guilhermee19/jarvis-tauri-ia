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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub use mic::{list_input_devices, Recorder, Recording};
pub use stt::transcribe;
pub use tts::{play, Chatterbox, TtsEngine, Voice};

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
    #[error("gravação curta demais — clique no microfone, fale, e clique de novo para parar")]
    GravacaoCurta,
    #[error(
        "não ouvi nada. Veja se o microfone certo é o padrão do Windows e se ele não está mudo"
    )]
    NadaOuvido,
    #[error(
        "o serviço de transcrição não respondeu em {0}. Confira o caminho do Whisper em Configurações"
    )]
    TranscricaoOffline(String),
    #[error("a transcrição passou de {0} s e foi cancelada — o whisper-server travou?")]
    TranscricaoDemorou(u64),
    #[error("a transcrição falhou (HTTP {status}): {corpo}")]
    TranscricaoRecusada { status: u16, corpo: String },
    #[error("falha de rede ao transcrever: {0}")]
    TranscricaoRede(String),
    #[error(
        "nenhum clipe de voz cadastrado. Escolha um arquivo com a sua voz em \
         Diagnóstico › Voz — bastam uns 10 segundos falando."
    )]
    NoVoiceAvailable,
    #[error("o servidor de voz recusou a chamada (HTTP {status}): {body}")]
    TtsRejected { status: u16, body: String },
    #[error("não consegui ler o clipe de voz: {0}")]
    ClipeIlegivel(String),
    #[error("falha de rede ao falar com o servidor de voz: {0}")]
    TtsNetwork(String),
    #[error("falha ao tocar o áudio: {0}")]
    Playback(String),
}

/// Dono do que é caro ou único: a gravação em andamento e o pool de conexões HTTP.
/// Registrado no Tauri como estado gerenciado, ao lado do `AppState`.
pub struct VoiceState {
    recorder: Mutex<Option<Recorder>>,
    http: reqwest::Client,
    cancelar_fala: Arc<AtomicBool>,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            recorder: Mutex::new(None),
            http: reqwest::Client::new(),
            cancelar_fala: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_recording(&self) -> bool {
        lock(&self.recorder).is_some()
    }

    /// Um pool só para os dois serviços locais — clonar compartilha as conexões.
    pub fn http(&self) -> reqwest::Client {
        self.http.clone()
    }

    pub fn start_recording<F>(&self, device_name: &str, on_level: F) -> Result<(), VoiceError>
    where
        F: Fn(f32) + Send + 'static,
    {
        let mut slot = lock(&self.recorder);
        if slot.is_some() {
            return Err(VoiceError::AlreadyRecording);
        }

        *slot = Some(Recorder::start(device_name, on_level)?);
        Ok(())
    }

    pub fn stop_recording(&self, path: &Path) -> Result<Recording, VoiceError> {
        let recorder = lock(&self.recorder)
            .take()
            .ok_or(VoiceError::NotRecording)?;
        recorder.stop(path)
    }

    /// Motor de TTS apontando para o servidor local.
    ///
    /// **Não devolve mais `Result`.** Enquanto era a ElevenLabs, esta fábrica existia
    /// principalmente para reclamar de uma API key vazia; sem chave para faltar, não sobrou
    /// nada aqui que possa dar errado. O que pode falhar é o servidor não estar de pé, e
    /// disso quem cuida é o `ensure_chatterbox`, antes desta chamada.
    pub fn tts(&self, base_url: &str) -> Box<dyn TtsEngine> {
        Box::new(Chatterbox::new(self.http.clone(), base_url))
    }

    /// Cadastra um clipe de voz no servidor e devolve o nome com que ele ficou lá.
    ///
    /// Passa por aqui, e não direto pelo comando, porque `Chatterbox` é privado ao módulo
    /// de voz — de fora só se vê a trait, e `enviar_referencia` não está nela (é cadastro,
    /// não síntese).
    pub async fn cadastrar_voz(
        &self,
        base_url: &str,
        caminho: &Path,
    ) -> Result<String, VoiceError> {
        Chatterbox::new(self.http.clone(), base_url)
            .enviar_referencia(caminho)
            .await
    }

    /// Limpa um "cala a boca" antigo e entrega a bandeira para o [`play`] desta fala.
    ///
    /// Zerar aqui, e não no fim da fala anterior, é o que faz o cancelamento valer
    /// também durante a SÍNTESE: mandar parar enquanto o modelo ainda está gerando marca
    /// a flag, e o áudio que chegar depois já nasce cancelado. Com um modelo local isso
    /// pesa mais do que pesava com a nuvem — a geração demora mais que o download.
    pub fn iniciar_fala(&self) -> Arc<AtomicBool> {
        self.cancelar_fala.store(false, Ordering::Relaxed);
        Arc::clone(&self.cancelar_fala)
    }

    pub fn parar_fala(&self) {
        self.cancelar_fala.store(true, Ordering::Relaxed);
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        Self::new()
    }
}
