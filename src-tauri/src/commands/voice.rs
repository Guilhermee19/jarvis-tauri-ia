use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::MotorDeVoz;
use crate::core::services::Services;
use crate::core::voice::{
    play, transcribe as transcribe_audio, Recording, Voice, VoiceError, VoiceState,
};
use crate::state::AppState;

/// Nível do microfone empurrado para a UI ~20×/s enquanto grava. É evento, e não
/// retorno de comando, porque a UI não pergunta: ela desenha o que chega.
const MIC_LEVEL_EVENT: &str = "jarvis://mic-level";

/// O mesmo, para o áudio que SAI. Irmão do de cima, e não um evento só com um campo
/// "fonte": os dois têm ciclos de vida independentes — o microfone publica entre
/// `start_recording` e `stop_recording`, a fala publica durante uma reprodução — e juntá-
/// -los obrigaria todo consumidor a filtrar algo que ele já sabe pelo nome.
const TTS_LEVEL_EVENT: &str = "jarvis://tts-level";

/// Nome do arquivo da última gravação. Um só, sobrescrito: é material de
/// diagnóstico, não histórico — a v0.2 lê este WAV, transcreve e descarta.
const RECORDING_FILE: &str = "ultima-gravacao.wav";

/// `(async)` porque abrir o dispositivo de entrada bloqueia por centenas de
/// milissegundos, e comando síncrono roda na thread principal do Tauri.
#[tauri::command(async)]
pub fn start_recording(
    app: AppHandle,
    voice: State<'_, VoiceState>,
    settings: State<'_, AppState>,
) -> Result<(), String> {
    let device_name = settings.settings().mic_device_name;
    voice
        .start_recording(&device_name, move |level| {
            let _ = app.emit(MIC_LEVEL_EVENT, level);
        })
        .map_err(stringify)
}

/// Os microfones disponíveis, para a configuração poder oferecer uma escolha.
///
/// `(async)` porque enumerar dispositivos de áudio conversa com o driver e bloqueia por
/// dezenas de milissegundos — mesma razão do `start_recording`.
///
/// O caminho vem qualificado de propósito: um `use` traria o nome do núcleo para este
/// escopo, onde já existe uma função com o mesmo nome, e a chamada viraria recursão
/// infinita em vez de erro de compilação.
#[tauri::command(async)]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    crate::core::voice::list_input_devices().map_err(stringify)
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

/// Sobe o servidor do motor pedido, se preciso, e devolve a URL dele.
///
/// Mesmo preâmbulo do `transcribe` com o Whisper, e pelo mesmo motivo: a subida é
/// preguiçosa, então quem chama qualquer coisa de voz é quem paga por ela estar de pé.
///
/// **Só um dos dois sobe.** Quem nunca sai do Piper nunca carrega o modelo do Chatterbox,
/// e vice-versa — é o mesmo princípio de "quem não usa a voz não paga nada", aplicado um
/// nível abaixo.
async fn servidor_de_voz(
    app: &AppHandle,
    http: &reqwest::Client,
    services: &Services,
    motor: MotorDeVoz,
) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("sem diretório de dados para achar o servidor de voz: {error}"))?;

    let subiu = match motor {
        MotorDeVoz::Piper => services.ensure_piper(http, &data_dir).await,
        MotorDeVoz::Chatterbox => services.ensure_chatterbox(http, &data_dir).await,
    };

    subiu.map_err(|error| error.to_string())
}

/// As vozes disponíveis no motor ativo — clipes cadastrados, no Chatterbox; vozes de
/// catálogo instaladas, no Piper.
#[tauri::command]
pub async fn list_voices(
    app: AppHandle,
    voice: State<'_, VoiceState>,
    state: State<'_, AppState>,
    services: State<'_, Services>,
) -> Result<Vec<Voice>, String> {
    let motor = state.settings().tts_engine;
    let http = voice.http();
    let url = servidor_de_voz(&app, &http, &services, motor).await?;

    voice.tts(motor, &url).voices().await.map_err(stringify)
}

/// Manda um `.wav`/`.mp3` da voz de alguém para o servidor e devolve o nome com que ele
/// ficou guardado — é esse nome que vai para as configurações da persona.
///
/// O caminho vem do seletor de arquivos nativo, então é um caminho real do disco desta
/// máquina. Ler o arquivo aqui e não do lado do servidor é o que mantém a porta aberta
/// para ele um dia não ser local.
#[tauri::command]
pub async fn upload_voice_reference(
    app: AppHandle,
    caminho: String,
    voice: State<'_, VoiceState>,
    services: State<'_, Services>,
) -> Result<String, String> {
    let http = voice.http();
    // Sempre o Chatterbox, mesmo com o Piper ativo: clipe de referência é coisa dele, e
    // subir o motor errado aqui daria um 404 confuso em vez de um cadastro.
    let url = servidor_de_voz(&app, &http, &services, MotorDeVoz::Chatterbox).await?;

    voice
        .cadastrar_voz(&url, std::path::Path::new(&caminho))
        .await
        .map_err(stringify)
}

/// `voice_id` vazio cai no clipe da persona ativa e, se ele também estiver vazio, no
/// primeiro clipe cadastrado. Quem chama de fora manda `speak_text(texto, None)` e não
/// precisa saber nada sobre onde os clipes moram.
#[tauri::command]
pub async fn speak_text(
    app: AppHandle,
    text: String,
    voice_id: Option<String>,
    voice: State<'_, VoiceState>,
    state: State<'_, AppState>,
    services: State<'_, Services>,
) -> Result<(), String> {
    let settings = state.settings();
    let http = voice.http();
    let url = servidor_de_voz(&app, &http, &services, settings.tts_engine).await?;
    let engine = voice.tts(settings.tts_engine, &url);

    let chosen = match voice_id.filter(|id| !id.trim().is_empty()) {
        Some(id) => id,
        // A voz da persona ATIVA: trocar de Jarvis para Ultron troca a voz junto, sem
        // reconfigurar nada — cada uma tem a sua guardada.
        None => resolve_voice(engine.as_ref(), settings.voz()).await?,
    };

    // Antes da síntese: mandar calar enquanto o modelo ainda está gerando tem que valer
    // para o áudio que está a caminho — e com um modelo local essa janela é bem maior do
    // que era com a nuvem.
    let cancelar = voice.iniciar_fala();
    let audio = engine.synthesize(&text, &chosen).await.map_err(stringify)?;

    // `play` bloqueia até o fim da fala. Fora do executor async isso travaria o
    // runtime do Tauri e, com ele, todos os outros comandos.
    tauri::async_runtime::spawn_blocking(move || {
        play(audio, cancelar, |level| {
            let _ = app.emit(TTS_LEVEL_EVENT, level);
        })
    })
        .await
        .map_err(|error| format!("a thread de áudio falhou: {error}"))?
        .map_err(stringify)
}

/// Cala a fala em andamento. Síncrono e sem erro de propósito: é o que o botão de
/// desligar o modo conversa chama, e nada aqui pode dar errado a ponto de valer uma
/// mensagem na tela — no pior caso não havia fala nenhuma para interromper.
#[tauri::command]
pub fn stop_speaking(voice: State<'_, VoiceState>) {
    voice.parar_fala();
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
