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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;
use serde::{Deserialize, Serialize};

use super::mic::{store_peak, LEVEL_INTERVAL};
use super::VoiceError;

const API_BASE: &str = "https://api.elevenlabs.io/v1";
/// O `flash` no lugar do `eleven_multilingual_v2`: fala português igual e sintetiza
/// em dezenas de milissegundos contra 1–2 s do multilíngue. Numa conversa por voz
/// esse tempo entra INTEIRO na espera do usuário, a cada frase, e a diferença de
/// expressividade entre os dois não paga isso. O multilíngue continua sendo a
/// escolha certa para narração longa, onde ninguém está esperando na frente.
const MODEL_ID: &str = "eleven_flash_v2_5";

/// Um `Source` que deixa as amostras passarem e anota a maior que viu.
///
/// **É a única costura possível para medir o que sai.** O `rodio` puxa amostras do
/// `Source` sob demanda do dispositivo e as entrega ao `Sink`; sem este embrulho no meio,
/// o áudio vai do MP3 decodificado direto para a placa e nenhuma linha do projeto chega
/// perto dele. Medir depois, no mixer, exigiria capturar a saída do sistema.
///
/// Não altera nada: `next` repassa a amostra intacta, e os quatro métodos do trait são
/// delegados. O custo é uma comparação atômica por amostra — a mesma que o microfone já
/// paga no callback dele.
struct ComPico<S> {
    fonte: S,
    pico: Arc<AtomicU32>,
}

impl<S: Source> Iterator for ComPico<S> {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let amostra = self.fonte.next()?;
        store_peak(&self.pico, amostra.abs());

        Some(amostra)
    }
}

impl<S: Source> Source for ComPico<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.fonte.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.fonte.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.fonte.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.fonte.total_duration()
    }
}

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
///
/// `on_level` recebe o pico do intervalo, de 0 a 1 — a mesma forma e a mesma cadência do
/// medidor do microfone, porque os dois alimentam a mesma animação. Callback e não evento
/// do Tauri pelo mesmo motivo de lá: o `core` não conhece o Tauri, e quem traduz isso em
/// evento é o `commands`.
pub fn play(
    audio: Vec<u8>,
    cancelar: Arc<AtomicBool>,
    on_level: impl Fn(f32),
) -> Result<(), VoiceError> {
    let stream = rodio::OutputStreamBuilder::open_default_stream().map_err(playback)?;
    let decoder = rodio::Decoder::new(Cursor::new(audio)).map_err(playback)?;

    let pico = Arc::new(AtomicU32::new(0));
    let sink = rodio::Sink::connect_new(stream.mixer());
    sink.append(ComPico {
        fonte: decoder,
        pico: Arc::clone(&pico),
    });

    // O `stream` tem que continuar vivo enquanto o sink toca — é por isso que a
    // espera fica aqui dentro, e não no chamador com o sink na mão.
    //
    // O mesmo laço serve para duas coisas: checar o cancelamento e publicar o nível. Eram
    // 100 ms só para o cancelamento; agora são os 50 ms do medidor, que é o que a
    // animação precisa e continua imperceptível para quem interrompe.
    while !sink.empty() {
        if cancelar.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        std::thread::sleep(LEVEL_INTERVAL);
        on_level(f32::from_bits(pico.swap(0, Ordering::Relaxed)));
    }

    // O zero final vale tanto para a fala que terminou quanto para a que foi cortada: sem
    // ele o último pico fica congelado na tela e o núcleo do HUD continua aceso depois
    // que o Jarvis calou.
    on_level(0.0);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Um `Source` de mentira, para o teste não depender de placa de som nem de MP3.
    struct Fita(std::vec::IntoIter<rodio::Sample>);

    impl Iterator for Fita {
        type Item = rodio::Sample;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next()
        }
    }

    impl Source for Fita {
        fn current_span_len(&self) -> Option<usize> {
            None
        }
        fn channels(&self) -> rodio::ChannelCount {
            1
        }
        fn sample_rate(&self) -> rodio::SampleRate {
            44_100
        }
        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    fn tocar(amostras: Vec<f32>) -> (Vec<f32>, f32) {
        let pico = Arc::new(AtomicU32::new(0));
        let mut fonte = ComPico {
            fonte: Fita(amostras.into_iter()),
            pico: Arc::clone(&pico),
        };

        let saida: Vec<f32> = std::iter::from_fn(|| fonte.next()).collect();

        (saida, f32::from_bits(pico.swap(0, Ordering::Relaxed)))
    }

    /// Sintetiza uma frase de verdade e imprime a série de níveis.
    ///
    /// Fora do `cargo test` comum porque gasta crédito da ElevenLabs, toca som na caixa e
    /// depende de rede. **É a única forma de provar que a amplitude acompanha a fala** —
    /// os testes de mesa acima provam que o embrulho mede, não que o que ele mede varia.
    ///
    /// ```text
    /// ELEVEN_KEY=… cargo test --lib -- --ignored --nocapture fala_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn fala_de_verdade() {
        let chave = std::env::var("ELEVEN_KEY").unwrap_or_default();
        let frase = std::env::var("ELEVEN_FRASE")
            .unwrap_or_else(|_| "Um, dois, três. Pausa. Quatro, cinco.".to_owned());

        let motor = ElevenLabs::new(reqwest::Client::new(), chave);

        let bloco = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let audio = bloco.block_on(async {
            let voz = match motor.voices().await {
                Ok(vozes) => vozes.first().map(|voz| voz.id.clone()).unwrap_or_default(),
                Err(erro) => {
                    println!("não listou as vozes: {erro}");
                    return None;
                }
            };

            match motor.synthesize(&frase, &voz).await {
                Ok(audio) => Some(audio),
                Err(erro) => {
                    println!("não sintetizou: {erro}");
                    None
                }
            }
        });

        let Some(audio) = audio else { return };
        println!("{} bytes de MP3", audio.len());

        let niveis = Arc::new(std::sync::Mutex::new(Vec::new()));
        let coletor = Arc::clone(&niveis);

        play(audio, Arc::new(AtomicBool::new(false)), move |nivel| {
            coletor.lock().expect("mutex").push(nivel);
        })
        .expect("tocou");

        let lidos = niveis.lock().expect("mutex").clone();
        let maior = lidos.iter().copied().fold(0.0_f32, f32::max);
        let barras: String = lidos
            .iter()
            .map(|nivel| {
                let altura = (nivel / maior.max(f32::EPSILON) * 7.0) as usize;
                ['.', '_', '-', '=', '+', '*', '#', '@'][altura.min(7)]
            })
            .collect();

        println!("{} amostras, pico {maior:.3}", lidos.len());
        println!("{barras}");
    }

    /// O embrulho não pode alterar o áudio: ele existe para MEDIR. Uma amostra trocada
    /// aqui viraria distorção na fala, e ninguém ligaria uma coisa à outra.
    #[test]
    fn as_amostras_passam_intactas() {
        let original = vec![0.1, -0.4, 0.25, 0.0];
        let (saida, _) = tocar(original.clone());

        assert_eq!(saida, original);
    }

    /// O pico é o maior valor ABSOLUTO: um estouro negativo é tão alto quanto um
    /// positivo, e ignorar o sinal faria a onda sumir na metade das sílabas.
    #[test]
    fn o_pico_e_o_maior_valor_absoluto() {
        let (_, pico) = tocar(vec![0.1, -0.8, 0.3]);

        assert!((pico - 0.8).abs() < f32::EPSILON, "pico foi {pico}");
    }

    /// O `swap` zera de propósito: cada leitura é o pico DAQUELE intervalo, não o da
    /// fala inteira. Sem isso a animação subiria uma vez e nunca mais desceria.
    #[test]
    fn a_leitura_zera_o_acumulador() {
        let pico = Arc::new(AtomicU32::new(0));
        let mut fonte = ComPico {
            fonte: Fita(vec![0.9_f32].into_iter()),
            pico: Arc::clone(&pico),
        };

        fonte.next();
        let primeira = f32::from_bits(pico.swap(0, Ordering::Relaxed));
        let segunda = f32::from_bits(pico.swap(0, Ordering::Relaxed));

        assert!((primeira - 0.9).abs() < f32::EPSILON);
        assert_eq!(segunda, 0.0, "sem amostra nova, o intervalo seguinte é silêncio");
    }
}
