use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::services::Services;
use crate::core::voice::{
    play, transcribe as transcribe_audio, Recording, Voice, VoiceError, VoiceState,
};
use crate::state::AppState;

/// Nível do microfone empurrado para a UI ~20×/s enquanto grava. É evento, e não
/// retorno de comando, porque a UI não pergunta: ela desenha o que chega.
const MIC_LEVEL_EVENT: &str = "jarvis://mic-level";

/// Nome do arquivo da última gravação. Um só, sobrescrito: é material de
/// diagnóstico, não histórico — a v0.2 lê este WAV, transcreve e descarta.
const RECORDING_FILE: &str = "ultima-gravacao.wav";

/// `(async)` porque abrir o dispositivo de entrada bloqueia por centenas de
/// milissegundos, e comando síncrono roda na thread principal do Tauri.
#[tauri::command(async)]
pub fn start_recording(app: AppHandle, voice: State<'_, VoiceState>) -> Result<(), String> {
    voice
        .start_recording(move |level| {
            let _ = app.emit(MIC_LEVEL_EVENT, level);
        })
        .map_err(stringify)
}

/// Devolve o WAV em disco em vez do áudio em si: é o formato que o Whisper quer, e
/// evita empurrar megabytes de PCM pelo IPC só para o frontend não usar.
#[tauri::command(async)]
pub fn stop_recording(app: AppHandle, voice: State<'_, VoiceState>) -> Result<Recording, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("sem diretório de cache para gravar o áudio: {error}"))?;

    voice
        .stop_recording(&dir.join(RECORDING_FILE))
        .map_err(stringify)
}

#[tauri::command]
pub fn is_recording(voice: State<'_, VoiceState>) -> bool {
    voice.is_recording()
}

/// Transcreve o WAV que `stop_recording` deixou em disco.
///
/// Separado do `stop_recording` de propósito: a bancada de diagnóstico testa o
/// microfone sem pagar os segundos do Whisper, e quem fala com o chat encadeia os
/// dois. É aqui que o whisper-server sobe, na primeira vez que alguém usa a voz —
/// quem nunca fala com o Jarvis nunca paga por isso.
#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    voice: State<'_, VoiceState>,
    services: State<'_, Services>,
) -> Result<String, String> {
    let http = voice.http();

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("sem diretório de dados para achar o Whisper: {error}"))?;
    let url = services
        .ensure_whisper(&http, &data_dir)
        .await
        .map_err(|error| error.to_string())?;

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("sem diretório de cache para ler o áudio: {error}"))?;

    transcribe_audio(&http, &url, &cache_dir.join(RECORDING_FILE))
        .await
        .map_err(stringify)
}

#[tauri::command]
pub async fn list_voices(
    voice: State<'_, VoiceState>,
    state: State<'_, AppState>,
) -> Result<Vec<Voice>, String> {
    let engine = voice
        .tts(&state.settings().eleven_labs_api_key)
        .map_err(stringify)?;
    engine.voices().await.map_err(stringify)
}

/// `voice_id` vazio cai na voz das configurações e, se ela também estiver vazia, na
/// primeira voz da conta. Assinatura pensada para o agente da v0.2+: ele chama
/// `speak_text(texto, None)` e não precisa saber nada sobre catálogo de vozes.
#[tauri::command]
pub async fn speak_text(
    text: String,
    voice_id: Option<String>,
    voice: State<'_, VoiceState>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings = state.settings();
    let engine = voice
        .tts(&settings.eleven_labs_api_key)
        .map_err(stringify)?;

    let chosen = match voice_id.filter(|id| !id.trim().is_empty()) {
        Some(id) => id,
        None => resolve_voice(engine.as_ref(), &settings.tts_voice_id).await?,
    };

    let audio = engine.synthesize(&text, &chosen).await.map_err(stringify)?;

    // `play` bloqueia até o fim da fala. Fora do executor async isso travaria o
    // runtime do Tauri e, com ele, todos os outros comandos.
    tauri::async_runtime::spawn_blocking(move || play(audio))
        .await
        .map_err(|error| format!("a thread de áudio falhou: {error}"))?
        .map_err(stringify)
}

async fn resolve_voice(
    engine: &dyn crate::core::voice::TtsEngine,
    configured: &str,
) -> Result<String, String> {
    if !configured.trim().is_empty() {
        return Ok(configured.to_owned());
    }

    let voices = engine.voices().await.map_err(stringify)?;
    voices
        .into_iter()
        .next()
        .map(|voice| voice.id)
        .ok_or_else(|| stringify(VoiceError::NoVoiceAvailable))
}

fn stringify(error: VoiceError) -> String {
    error.to_string()
}
