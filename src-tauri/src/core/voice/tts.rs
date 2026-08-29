//! Síntese de fala.
//!
//! O motor é o **Chatterbox**, rodando na própria máquina: um modelo aberto da Resemble AI
//! (licença MIT) que **clona uma voz a partir de um clipe de ~10 s**, sem treino nenhum.
//! Quem sobe e derruba o processo é `crate::core::services`; daqui para baixo é só HTTP
//! contra o `localhost`, do mesmo jeito que o `stt.rs` fala com o whisper-server.
//!
//! Aqui morava a ElevenLabs. Ela saiu inteira, e a aposta da doc antiga se pagou: como o
//! acesso sempre passou por [`TtsEngine`], trocar o motor não encostou em [`play`], nem
//! nos comandos, nem nos wrappers do frontend.
//!
//! ## Por que um processo Python, se o resto do projeto foge disso
//!
//! Não há caminho puro-Rust. A única variante do Chatterbox com export ONNX é a **Turbo**,
//! e ela **fala só inglês**; português exige a Multilingual, que só existe em PyTorch. O
//! `ROADMAP` evitava sidecar Python em favor de crates nativas — aqui não teve como, e a
//! troca é consciente.

use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;
use serde::{Deserialize, Serialize};

use super::mic::{store_peak, LEVEL_INTERVAL};
use super::VoiceError;

/// Idioma pedido ao modelo multilíngue.
///
/// Constante, e não configuração: o Whisper já sobe com `-l pt` fixo, os prompts são em
/// português e as personas também. Uma voz em outro idioma não seria uma opção — seria um
/// defeito, e um que ninguém iria procurar nas Configurações.
const IDIOMA: &str = "pt";

/// WAV, e não MP3.
///
/// É o formato que o servidor produz sem depender de um ffmpeg instalado, e o `rodio`
/// decodifica os dois de qualquer jeito (a feature `wav` vem entre as padrão). Sobre
/// loopback, os bytes a mais não custam nada.
const FORMATO: &str = "wav";

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
    /// Devolve o áudio codificado (WAV, no Chatterbox), sem tocar: quem toca é
    /// [`play`], para que o áudio também possa ser salvo ou testado sem som.
    async fn synthesize(&self, text: &str, voice_id: &str) -> Result<Vec<u8>, VoiceError>;
}

/// O servidor local do Chatterbox.
pub struct Chatterbox {
    http: reqwest::Client,
    /// Já sem a barra final, para as rotas serem concatenadas sem cuidado extra.
    base: String,
}

impl Chatterbox {
    pub fn new(http: reqwest::Client, base: &str) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    fn rota(&self, caminho: &str) -> String {
        format!("{}{caminho}", self.base)
    }

    /// Guarda um clipe de voz no servidor e devolve o nome com que ele ficou lá.
    ///
    /// **Fora da trait de propósito**: isto é cadastro, não síntese. Um motor que não
    /// clonasse voz nenhuma continuaria implementando [`TtsEngine`] inteiro sem ter que
    /// inventar o que responder aqui.
    ///
    /// O nome devolvido é o que vai para `ttsVoiceJarvis`/`ttsVoiceUltron` e volta depois
    /// como `reference_audio_filename` — é a única cola entre o clipe no disco do servidor
    /// e a configuração do app.
    pub async fn enviar_referencia(&self, caminho: &Path) -> Result<String, VoiceError> {
        let nome = caminho
            .file_name()
            .and_then(|nome| nome.to_str())
            .ok_or_else(|| VoiceError::ClipeIlegivel("o arquivo não tem nome".to_owned()))?
            .to_owned();

        // Lido aqui e enviado como bytes: mandar só o caminho funcionaria hoje, com o
        // servidor na mesma máquina, e quebraria no dia em que ele não estivesse.
        let bytes =
            std::fs::read(caminho).map_err(|erro| VoiceError::ClipeIlegivel(erro.to_string()))?;

        // O campo é `files`, no plural, porque o endpoint recebe uma lista. Mandamos um.
        let formulario = reqwest::multipart::Form::new().part(
            "files",
            reqwest::multipart::Part::bytes(bytes).file_name(nome.clone()),
        );

        let resposta = self
            .http
            .post(self.rota("/upload_reference"))
            .multipart(formulario)
            .send()
            .await
            .map_err(network)?;

        let recibo: Recibo = check(resposta).await?.json().await.map_err(network)?;

        // O servidor higieniza o nome do arquivo, então o que ele guardou pode não ser o
        // que mandamos — quem manda é a resposta dele, não o nosso `nome`.
        let Recibo {
            uploaded_files,
            message,
        } = recibo;

        uploaded_files
            .into_iter()
            .next()
            .ok_or(VoiceError::ClipeIlegivel(message))
    }
}

#[async_trait::async_trait]
impl TtsEngine for Chatterbox {
    /// As "vozes" são os clipes de referência que já foram enviados.
    ///
    /// A rota devolve uma lista crua de nomes de arquivo — sem id, sem descrição. É bem
    /// menos do que o catálogo da ElevenLabs trazia, e é tudo o que a tela precisa.
    async fn voices(&self) -> Result<Vec<Voice>, VoiceError> {
        let resposta = self
            .http
            .get(self.rota("/get_reference_files"))
            .send()
            .await
            .map_err(network)?;

        let arquivos: Vec<String> = check(resposta).await?.json().await.map_err(network)?;

        Ok(arquivos
            .into_iter()
            .map(|arquivo| Voice {
                name: sem_extensao(&arquivo),
                id: arquivo,
                description: None,
            })
            .collect())
    }

    async fn synthesize(&self, text: &str, voice_id: &str) -> Result<Vec<u8>, VoiceError> {
        let resposta = self
            .http
            .post(self.rota("/tts"))
            .json(&PedidoDeFala {
                text,
                voice_mode: "clone",
                reference_audio_filename: voice_id,
                language: IDIOMA,
                output_format: FORMATO,
            })
            // Sem timeout próprio: a primeira frase depois de abrir o app espera o modelo
            // subir para a VRAM, e cortar a chamada no meio disso viraria um erro sem
            // causa visível no lugar de uma demora explicável.
            .send()
            .await
            .map_err(network)?;

        let audio = check(resposta).await?.bytes().await.map_err(network)?;
        Ok(audio.to_vec())
    }
}

/// Só os campos que o Jarvis decide.
///
/// Todo o resto (`split_text`, `chunk_size`, `seed`, `exaggeration`, `cfg_weight`) fica no
/// padrão do servidor de propósito: são botões de laboratório, e fixá-los aqui congelaria
/// escolhas que ninguém deste lado tem como avaliar.
#[derive(Serialize)]
struct PedidoDeFala<'a> {
    text: &'a str,
    voice_mode: &'a str,
    reference_audio_filename: &'a str,
    language: &'a str,
    output_format: &'a str,
}

#[derive(Deserialize)]
struct Recibo {
    #[serde(default)]
    uploaded_files: Vec<String>,
    /// Só é lida quando `uploaded_files` vem vazia — aí ela é a explicação da recusa.
    #[serde(default)]
    message: String,
}

/// `minha-voz.wav` vira `minha-voz`. A extensão é ruído na lista da tela.
fn sem_extensao(arquivo: &str) -> String {
    arquivo
        .rsplit_once('.')
        .map_or(arquivo, |(nome, _)| nome)
        .to_owned()
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

/// Erro do servidor com o corpo junto: sem ele, um "422" não diz se o clipe de referência
/// sumiu do disco, se o idioma não existe no modelo carregado, ou se o texto veio vazio.
///
/// Truncado em 300 caracteres, como no `stt.rs`: o FastAPI responde erro de validação com
/// um JSON longo, e o que interessa está no começo dele.
async fn check(resposta: reqwest::Response) -> Result<reqwest::Response, VoiceError> {
    let status = resposta.status();
    if status.is_success() {
        return Ok(resposta);
    }

    let body: String = resposta
        .text()
        .await
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();

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

    /// O corpo do `POST /tts` é um contrato com um servidor Python que este projeto não
    /// compila. Um campo renomeado aqui não quebra build nenhum: vira um 422 em tempo de
    /// execução, ou — pior — um pedido aceito que ignora silenciosamente o clipe de voz e
    /// responde com a voz padrão do modelo.
    #[test]
    fn o_pedido_de_fala_sai_com_os_campos_que_o_servidor_espera() {
        let pedido = PedidoDeFala {
            text: "oi",
            voice_mode: "clone",
            reference_audio_filename: "minha-voz.wav",
            language: IDIOMA,
            output_format: FORMATO,
        };

        assert_eq!(
            serde_json::to_value(&pedido).expect("json"),
            serde_json::json!({
                "text": "oi",
                "voice_mode": "clone",
                "reference_audio_filename": "minha-voz.wav",
                "language": "pt",
                "output_format": "wav",
            }),
        );
    }

    /// O nome que vale é o que o SERVIDOR devolveu, não o do arquivo que mandamos: ele
    /// higieniza o nome, e guardar o nosso deixaria a configuração apontando para um
    /// clipe que não existe do lado de lá.
    #[test]
    fn o_recibo_do_upload_diz_o_nome_que_ficou_guardado() {
        let recibo: Recibo = serde_json::from_str(
            r#"{"message":"ok","uploaded_files":["minha_voz.wav"],
                "all_reference_files":["minha_voz.wav"],"errors":[]}"#,
        )
        .expect("recibo");

        assert_eq!(recibo.uploaded_files, ["minha_voz.wav"]);
    }

    /// Recusa devolve 200 com a lista vazia e o motivo na `message` — não é erro de HTTP,
    /// então o `check` deixa passar e quem tem que perceber é o código que lê o recibo.
    #[test]
    fn recibo_sem_arquivo_ainda_traz_o_motivo() {
        let recibo: Recibo =
            serde_json::from_str(r#"{"message":"formato não suportado","uploaded_files":[]}"#)
                .expect("recibo");

        assert!(recibo.uploaded_files.is_empty());
        assert_eq!(recibo.message, "formato não suportado");
    }

    #[test]
    fn a_lingueta_mostra_o_clipe_sem_a_extensao() {
        assert_eq!(sem_extensao("minha-voz.wav"), "minha-voz");
        assert_eq!(sem_extensao("gravacao.final.mp3"), "gravacao.final");
        // Sem extensão não é caso comum, mas devolver vazio seria pior que devolver feio.
        assert_eq!(sem_extensao("sem_ponto"), "sem_ponto");
    }

    /// A URL vem do `ensure_chatterbox`, que a monta com `format!` — mas nada impede
    /// alguém de escrevê-la à mão com barra no fim, e aí toda rota viraria `//tts`.
    #[test]
    fn a_barra_final_da_url_nao_dobra_nas_rotas() {
        let motor = Chatterbox::new(reqwest::Client::new(), "http://127.0.0.1:8004/");
        assert_eq!(motor.rota("/tts"), "http://127.0.0.1:8004/tts");
    }

    /// Sintetiza uma frase de verdade, **cronometra**, e imprime a série de níveis.
    ///
    /// Fora do `cargo test` comum porque exige o servidor do Chatterbox de pé e toca som na
    /// caixa. Não é um teste: é a ferramenta de medição da feature, e produz os dois
    /// números que ninguém consegue deduzir lendo código.
    ///
    /// 1. **Quantos segundos até a primeira palavra.** Numa RTX 2060 com o Multilingual,
    ///    uma frase de 52 caracteres levou **6,6 a 8,1 s** já quente — mais devagar que
    ///    tempo real. O `stream: true` do servidor foi medido e é PIOR (8,9 s até o
    ///    primeiro byte): ele fatia por trecho, e uma frase é um trecho só. Numa GPU
    ///    diferente esse número muda, e é por isso que a ferramenta continua aqui.
    /// 2. **O pico típico da fala**, que calibra o `PICO_TIPICO_DA_FALA` do
    ///    `useVoiceInput.ts` — hoje 0,28, medido aqui. Ele depende do volume do CLIPE de
    ///    referência, porque o modelo clona o volume junto com a voz: trocar de clipe
    ///    pede rodar isto de novo.
    ///
    /// ```text
    /// JARVIS_TTS_VOZ=minha-voz.wav cargo test --lib -- --ignored --nocapture fala_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn fala_de_verdade() {
        let base = std::env::var("JARVIS_TTS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8004".to_owned());
        let frase = std::env::var("JARVIS_TTS_FRASE")
            .unwrap_or_else(|_| "Um, dois, três. Pausa. Quatro, cinco.".to_owned());

        let motor = Chatterbox::new(reqwest::Client::new(), &base);

        let bloco = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let audio = bloco.block_on(async {
            // Sem voz no ambiente, usa o primeiro clipe cadastrado — assim a ferramenta
            // roda com uma variável a menos para quem só quer ver se está de pé.
            let voz = match std::env::var("JARVIS_TTS_VOZ") {
                Ok(voz) if !voz.trim().is_empty() => voz,
                _ => match motor.voices().await {
                    Ok(vozes) if !vozes.is_empty() => vozes[0].id.clone(),
                    Ok(_) => {
                        println!("nenhum clipe de referência cadastrado no servidor");
                        return None;
                    }
                    Err(erro) => {
                        println!("não listou os clipes: {erro}");
                        return None;
                    }
                },
            };

            println!("voz: {voz}");
            let relogio = std::time::Instant::now();

            match motor.synthesize(&frase, &voz).await {
                Ok(audio) => {
                    // O número que decide se esta feature vale a pena.
                    println!(
                        "sintetizou em {:.2} s ({} caracteres)",
                        relogio.elapsed().as_secs_f32(),
                        frase.chars().count()
                    );
                    Some(audio)
                }
                Err(erro) => {
                    println!("não sintetizou: {erro}");
                    None
                }
            }
        });

        let Some(audio) = audio else { return };
        println!("{} bytes de WAV", audio.len());

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
