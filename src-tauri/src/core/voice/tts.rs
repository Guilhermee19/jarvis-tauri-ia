//! Síntese de fala.
//!
//! O motor é a ElevenLabs porque é HTTP puro: nada de binário nem modelo dentro do
//! bundle, e o catálogo de vozes vem do próprio serviço (o Piper exigiria empacotar
//! o executável por plataforma e um `.onnx` por voz).
//!
//! O preço disso é depender de rede e de crédito pago — e é exatamente por isso que
//! o acesso passa por [`TtsEngine`]: trocar por Piper local depois é escrever outra
//! impl, sem tocar em `commands::voice::speak_text`.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::VoiceError;

const API_BASE: &str = "https://api.elevenlabs.io/v1";
/// O `flash` no lugar do `eleven_multilingual_v2`: fala português igual e sintetiza
/// em dezenas de milissegundos contra 1–2 s do multilíngue. Numa conversa por voz
/// esse tempo entra INTEIRO na espera do usuário, a cada frase, e a diferença de
/// expressividade entre os dois não paga isso. O multilíngue continua sendo a
/// escolha certa para narração longa, onde ninguém está esperando na frente.
const MODEL_ID: &str = "eleven_flash_v2_5";

/// De quanto em quanto tempo a fala em andamento checa se mandaram calar. 100 ms é
/// imperceptível para quem interrompe e não faz a thread girar à toa.
const CHECAGEM_DE_CANCELAMENTO: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: String,
    pub name: String,
    /// Descrição ou categoria do catálogo — ajuda a escolher na UI.
    pub description: Option<String>,
}

#[async_trait::async_trait]
pub trait TtsEngine: Send + Sync {
    async fn voices(&self) -> Result<Vec<Voice>, VoiceError>;
    /// Devolve o áudio codificado (MP3 na ElevenLabs), sem tocar: quem toca é
    /// [`play`], para que o áudio também possa ser salvo ou testado sem som.
    async fn synthesize(&self, text: &str, voice_id: &str) -> Result<Vec<u8>, VoiceError>;
}

pub struct ElevenLabs {
    http: reqwest::Client,
    api_key: String,
}

impl ElevenLabs {
    pub fn new(http: reqwest::Client, api_key: String) -> Self {
        Self { http, api_key }
    }
}

#[async_trait::async_trait]
impl TtsEngine for ElevenLabs {
    async fn voices(&self) -> Result<Vec<Voice>, VoiceError> {
        let response = self
            .http
            .get(format!("{API_BASE}/voices"))
            .header("xi-api-key", &self.api_key)
            .send()
            .await
            .map_err(network)?;

        let catalog: VoiceCatalog = check(response).await?.json().await.map_err(network)?;

        Ok(catalog
            .voices
            .into_iter()
            .map(|voice| Voice {
                id: voice.voice_id,
                name: voice.name,
                description: voice.description.or(voice.category),
            })
            .collect())
    }

    async fn synthesize(&self, text: &str, voice_id: &str) -> Result<Vec<u8>, VoiceError> {
        let response = self
            .http
            .post(format!("{API_BASE}/text-to-speech/{voice_id}"))
            .header("xi-api-key", &self.api_key)
            .json(&SynthesisRequest {
                text,
                model_id: MODEL_ID,
            })
            .send()
            .await
            .map_err(network)?;

        let audio = check(response).await?.bytes().await.map_err(network)?;
        Ok(audio.to_vec())
    }
}

#[derive(Serialize)]
struct SynthesisRequest<'a> {
    text: &'a str,
    model_id: &'a str,
}

#[derive(Deserialize)]
struct VoiceCatalog {
    voices: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    voice_id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

/// Toca o áudio e só volta quando ele termina. Bloqueia de propósito: quem chama —
/// o botão de teste e o laço do modo conversa — precisa saber quando o Jarvis calou,
/// porque é aí que o microfone reabre. Por bloquear, tem que rodar fora do executor
/// async (veja `commands::voice`).
///
/// `cancelar` é o que faz "desliga o modo conversa" calar na hora. O
/// `sink.sleep_until_end()` que estava aqui não tem como ser interrompido: quem
/// mandasse parar no meio de uma resposta longa continuaria ouvindo por meia frase,
/// e o microfone só reabriria depois.
pub fn play(audio: Vec<u8>, cancelar: Arc<AtomicBool>) -> Result<(), VoiceError> {
    let stream = rodio::OutputStreamBuilder::open_default_stream().map_err(playback)?;
    let decoder = rodio::Decoder::new(Cursor::new(audio)).map_err(playback)?;

    let sink = rodio::Sink::connect_new(stream.mixer());
    sink.append(decoder);

    // O `stream` tem que continuar vivo enquanto o sink toca — é por isso que a
    // espera fica aqui dentro, e não no chamador com o sink na mão.
    while !sink.empty() {
        if cancelar.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        std::thread::sleep(CHECAGEM_DE_CANCELAMENTO);
    }

    Ok(())
}

/// Erro da ElevenLabs traduzido com o corpo junto: sem ele, "401" não diz se é key
/// errada, cota estourada ou voz inexistente.
///
/// Permissão faltando ganha caso próprio pelo mesmo motivo do `classify` do `mic.rs`:
/// o texto cru diz `missing the permission voices_read`, o que é exato e não ajuda
/// nada — a saída é marcar a permissão na key OU escolher uma voz, e nenhuma das duas
/// está na resposta da API.
async fn check(response: reqwest::Response) -> Result<reqwest::Response, VoiceError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body: String = response
        .text()
        .await
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();

    if body.contains("missing_permissions") {
        return Err(VoiceError::TtsSemPermissao(body));
    }

    Err(VoiceError::TtsRejected {
        status: status.as_u16(),
        body,
    })
}

fn network(error: reqwest::Error) -> VoiceError {
    VoiceError::TtsNetwork(error.to_string())
}

fn playback(error: impl std::fmt::Display) -> VoiceError {
    VoiceError::Playback(error.to_string())
}
