//! O agente: entende a frase, age, conversa e aprende.
//!
//! O ciclo é curto de propósito — **uma** chamada ao roteador ([`intent`]) e, quando é
//! papo em vez de comando, mais duas em [`converse`] (responder e extrair memória).
//! Não é o loop de tool use da Anthropic; quando ele entrar, entra ao lado, atrás
//! desta mesma função [`handle`].
//!
//! Saem daqui duas coisas: a resposta que o usuário lê, e o LOG do gatilho — o que foi
//! ouvido, o que o modelo entendeu, no que deu, e o que entrou ou saiu da memória. O
//! log existe porque um assistente que abre programas e guarda fatos sobre você erra
//! em silêncio se ninguém puder auditar o que ele achou que foi pedido.

mod converse;
mod intent;

use std::time::Instant;

use chrono::Utc;

pub use intent::client;

/// Reexportados para `core::vision`, que fala com o MESMO modelo já quente — em 4 GB
/// de VRAM não cabe um segundo.
pub(crate) use intent::{pedir as pedir_ao_modelo, KEEP_ALIVE};

use intent::Intent;

use crate::config::AppSettings;
use crate::core::automation::AutomationState;
use crate::core::memory::{Acao, Memoria};
use crate::core::music;
use crate::core::search;
use crate::core::system::{self, MediaKey, SystemError};
use crate::core::vision;

/// Um passo de volume. A tecla do Windows anda ~2%, o que é imperceptível quando
/// alguém fala "aumenta o volume" — em comando de voz um passo precisa ser um passo.
const PASSO_DE_VOLUME: i8 = 10;

/// Quantas mensagens antigas viram resumo de uma vez.
const LOTE_DE_RESUMO: usize = 30;

const NOTA_DO_RESUMO: &str = "resumo das conversas";

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(
        "não achei o Ollama em {url}. Instale de ollama.com, deixe ele rodando, e baixe o modelo com `ollama pull {model}`"
    )]
    Offline { url: String, model: String },
    #[error("o Ollama não tem o modelo {0} baixado — rode `ollama pull {0}` no terminal")]
    ModeloAusente(String),
    #[error("o Ollama recusou a chamada (HTTP {status}): {corpo}")]
    Recusado { status: u16, corpo: String },
    #[error(
        "o modelo demorou demais para responder — na primeira chamada ele carrega na memória e isso leva mais de um minuto"
    )]
    Demorou,
    #[error("falha de rede ao falar com o Ollama: {0}")]
    Rede(String),
    #[error("o modelo devolveu algo que não é uma ação válida: {0}")]
    NaoEntendi(String),
    #[error(
        "o modelo {0} não enxerga imagem. Troque por um multimodal em Configurações — `ollama pull qwen2.5vl:3b` e ponha `qwen2.5vl:3b` no campo do modelo"
    )]
    SemVisao(String),
    #[error("não consegui pegar a imagem da câmera: {0}")]
    SemCamera(String),
}

/// Coisa que só a UI sabe fazer.
///
/// A câmera é o caso: quem é dono do laço de preview é o `sensorStore`, não o Rust.
/// Abrir o dispositivo aqui deixaria a câmera ligada com o botão apagado e a tela
/// vazia — o estado da UI é que manda. Então o agente PEDE, e a UI faz exatamente o
/// que o botão faria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcaoDeUi {
    AbrirWebcam,
    FecharWebcam,
}

impl AcaoDeUi {
    /// O que viaja no evento. Espelhado em `src/lib/tauri/events.ts`.
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::AbrirWebcam => "webcam-on",
            Self::FecharWebcam => "webcam-off",
        }
    }
}

/// O que [`handle`] devolve: a fala, e o log quando houve comando ou mudança na memória.
pub struct Outcome {
    /// `None` em conversa que não mexeu em nada. Uma caixa de log embaixo de cada
    /// "bom dia" é o ruído que faz o log inteiro passar a ser ignorado.
    pub trace: Option<String>,
    pub reply: String,
    /// `Some` quando o comando só se completa do lado da UI.
    pub ui: Option<AcaoDeUi>,
}

pub async fn handle(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    automation: &AutomationState,
    dito: &str,
) -> Result<Outcome, AgentError> {
    // Modelo vazio desliga o intérprete e volta ao mock. É a saída de emergência sem
    // precisar de um booleano — mesmo padrão do `tts_voice_id` ("vazio = padrão").
    if settings.ollama_model.trim().is_empty() {
        return Ok(Outcome {
            trace: None,
            reply: crate::core::chat::mock_reply_text(&settings.assistant_name, dito),
            ui: None,
        });
    }

    // Você pode ter editado as notas no Obsidian com o app aberto.
    memoria.recarregar();

    let url = &settings.ollama_url;
    let model = &settings.ollama_model;
    let nome = &settings.assistant_name;

    let relogio = Instant::now();
    let acao = intent::interpret(http, url, model, nome, &memoria.apelidos(), dito).await?;
    let pensou = relogio.elapsed();

    let mut log = Log::novo(dito, model, pensou);
    let mut ui = None;

    let reply = match &acao {
        Intent::Reply {} => conversar(http, settings, memoria, dito, &mut log).await?,

        // Pergunta sobre o mundo: dá uma olhada na internet, GUARDA o que achou, e
        // responde conversando. A aba do navegador só abre quando ele mandou pesquisar
        // — abrir Google no meio de um papo é interromper, não ajudar.
        Intent::WebSearch { query } => {
            log.acao(&acao);

            if pediu_a_aba(dito) {
                let relogio = Instant::now();
                let abriu = system::search_web(query).map(|()| String::new());
                log.resultado(&abriu, relogio.elapsed().as_millis());
            }

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            pesquisar_e_responder(http, settings, memoria, query, &mut log).await
        }

        // Tocar uma música nomeada. Sobe o Spotify se estiver fechado — o `spotify:`
        // é registrado pelo app, então o ShellExecute resolve.
        Intent::PlayMusic { query } => {
            log.acao(&acao);

            let relogio = Instant::now();
            let tocando = music::tocar(
                http,
                query,
                &settings.spotify_client_id,
                &settings.spotify_client_secret,
            )
            .await;
            let levou = relogio.elapsed().as_millis();

            log.linhas.push(format!(
                "MÚSICA     spotify · {}",
                music::modo(&settings.spotify_client_id, &settings.spotify_client_secret)
            ));
            log.desfecho(tocando.as_ref().err().map(ToString::to_string), levou);

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: tocando.is_ok(),
            });
            memoria.atualizar_rotinas();

            match tocando {
                Ok(tocando) => match tocando.faixa {
                    Some(faixa) => format!("Tocando {faixa}."),
                    None => format!(
                        "Abri a busca por \"{query}\" no Spotify — é só dar play. Para eu \
                         tocar direto, ponha as credenciais do Spotify em Configurações."
                    ),
                },
                Err(erro) => erro.to_string(),
            }
        }

        // A câmera é da UI. O agente registra a ação e pede; quem liga é o
        // `sensorStore`, pelo mesmo caminho do botão da barra de ícones.
        Intent::WebcamOn {} | Intent::WebcamOff {} => {
            log.acao(&acao);

            let ligar = matches!(acao, Intent::WebcamOn {});
            ui = Some(if ligar {
                AcaoDeUi::AbrirWebcam
            } else {
                AcaoDeUi::FecharWebcam
            });

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: true,
            });
            memoria.atualizar_rotinas();

            if ligar {
                "Ligando a câmera.".to_owned()
            } else {
                "Câmera desligada.".to_owned()
            }
        }

        // Olhar pela câmera. A captura vem PRIMEIRO e o pedido de mostrar depois: se
        // fosse ao contrário, a UI e o Rust disputariam a abertura do dispositivo.
        // `capture_webcam_frame` abre e fecha sozinho quando a câmera está desligada
        // — foi desenhado para exatamente este caso.
        Intent::Look {} => {
            log.acao(&acao);

            let relogio = Instant::now();
            let resultado = olhar(http, settings, automation).await;
            log.desfecho(
                resultado.as_ref().err().map(ToString::to_string),
                relogio.elapsed().as_millis(),
            );

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: resultado.is_ok(),
            });
            memoria.atualizar_rotinas();

            match resultado {
                Ok(descricao) => {
                    // Só mostra a câmera se deu certo — ligar a webcam para em seguida
                    // dizer "não consegui ver" é o pior dos dois mundos.
                    ui = Some(AcaoDeUi::AbrirWebcam);
                    descricao
                }
                Err(erro) => erro.to_string(),
            }
        }

        // ---- memória explícita: o caminho confiável ----------------------
        Intent::Remember { fact } => {
            log.acao(&acao);
            if memoria.lembrar(&assunto_de(fact), fact) {
                log.memoria('+', &assunto_de(fact));
                "Guardado.".to_owned()
            } else {
                "Isso eu já sabia.".to_owned()
            }
        }
        Intent::Forget { about } => {
            log.acao(&acao);
            let apagadas = memoria.esquecer(about);
            if apagadas.is_empty() {
                format!("Não achei nada sobre \"{about}\" na memória.")
            } else {
                for nome in &apagadas {
                    log.memoria('-', nome);
                }
                format!("Esqueci {}.", apagadas.join(", "))
            }
        }
        Intent::Alias { nickname, target } => {
            log.acao(&acao);
            if memoria.apelidar(nickname, target) {
                log.memoria('+', nickname);
                format!("Combinado: \"{nickname}\" é o {target}.")
            } else {
                "Isso eu já sabia.".to_owned()
            }
        }

        // ---- comandos do PC ----------------------------------------------
        _ => {
            log.acao(&acao);
            let relogio = Instant::now();
            let resultado = execute(&acao);
            log.resultado(&resultado, relogio.elapsed().as_millis());

            memoria.registrar_acao(Acao {
                quando: Utc::now().timestamp_millis(),
                acao: verbo(&acao),
                alvo: argumentos(&acao),
                ok: resultado.is_ok(),
            });
            memoria.atualizar_rotinas();

            // Falha de execução responde como FRASE, não como erro de IPC: "não achei
            // o Spotify" é uma conversa, não o backend caindo.
            match resultado {
                Ok(frase) => frase,
                Err(erro) => erro.to_string(),
            }
        }
    };

    Ok(Outcome {
        trace: log.render(),
        reply,
        ui,
    })
}

/// Papo: responde com histórico e memória, e depois tenta aprender algo com o que foi
/// dito. As duas chamadas são separadas porque um 3B não faz as duas coisas numa só —
/// o porquê, com os números, está no topo de [`converse`].
async fn conversar(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    dito: &str,
    log: &mut Log,
) -> Result<String, AgentError> {
    let url = &settings.ollama_url;
    let model = &settings.ollama_model;

    let resposta = converse::responder(
        http,
        url,
        model,
        &settings.assistant_name,
        &memoria.contexto(dito),
        &memoria.recentes(converse::JANELA),
        dito,
    )
    .await?;

    destilar(http, settings, memoria, dito, &resposta, log).await;
    talvez_resumir(http, settings, memoria).await;
    Ok(resposta)
}

/// Entende do que a troca tratou e escreve a nota de conhecimento sobre aquele assunto.
///
/// Duas chamadas, e a divisão é o que faz a nota virar documento: a primeira só decide
/// O TEMA, a segunda recebe o que já estava escrito e devolve a nota inteira reescrita.
/// Numa chamada só, sem o texto anterior na frente, o modelo não teria como fundir — e
/// o resultado seria a pilha de frases coladas que a nota não deve ser.
///
/// BEST-EFFORT do começo ao fim: qualquer falha aqui é engolida, porque a resposta ao
/// usuário já foi composta e perder uma nota é recuperável ("lembra que...", que passa
/// pelo roteador e é confiável).
async fn destilar(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    dito: &str,
    resposta: &str,
    log: &mut Log,
) {
    let url = &settings.ollama_url;
    let model = &settings.ollama_model;

    // A troca inteira, não só a frase do usuário: metade do conhecimento aparece na
    // explicação que o assistente deu, e a versão anterior jogava isso fora.
    let troca = format!("Usuário: {dito}\nAssistente: {resposta}");
    let indice = memoria.nomes_das_notas();

    let Ok(Some(assunto)) = converse::destilar_assunto(http, url, model, &indice, &troca).await
    else {
        return;
    };

    let atual = memoria.corpo_da_nota(&assunto);
    let Ok(nota) =
        converse::escrever_nota(http, url, model, &assunto, &atual, &troca, &indice).await
    else {
        return;
    };

    // Nota igual à anterior é o caso de "a conversa não acrescentou nada" — o próprio
    // prompt manda devolver como está. Regravar só sujaria o `atualizado` e o git.
    if nota.trim().is_empty() || nota.trim() == atual.trim() {
        return;
    }

    memoria.escrever_conhecimento(&assunto, &nota);
    log.memoria(if atual.is_empty() { '+' } else { '~' }, &assunto);
}

/// Tira um quadro da webcam, descreve, e tenta enriquecer com uma busca.
///
/// A busca é BEST-EFFORT de propósito: ela é o "tenta entender" do pedido, mas se a
/// internet estiver fora ou a Wikipedia não souber do assunto, dizer o que se vê já
/// vale — e é o que foi pedido primeiro.
async fn olhar(
    http: &reqwest::Client,
    settings: &AppSettings,
    automation: &AutomationState,
) -> Result<String, AgentError> {
    // ponytail: chamada bloqueante dentro de `async`. Com a câmera já aberta o `grab`
    // volta em milissegundos; fechada, o `Session::open` custa algumas centenas. Vira
    // `spawn_blocking` se algum dia travar a UI de verdade.
    let quadro = automation
        .capture_webcam_frame()
        .map_err(|erro| AgentError::SemCamera(erro.to_string()))?;

    let descricao = vision::descrever(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        vision::so_o_base64(&quadro.data_url),
    )
    .await?;

    let Ok(achados) = search::pesquisar(http, &descricao, &settings.brave_api_key).await else {
        return Ok(descricao);
    };

    let enriquecida = converse::responder_sobre_o_que_viu(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &settings.assistant_name,
        &descricao,
        &achados,
    )
    .await;

    Ok(enriquecida.unwrap_or(descricao))
}

/// Frases que pedem a aba do navegador. O resto é pergunta no meio da conversa, e
/// abrir o Google nessas seria interromper.
const ORDENS_DE_BUSCA: [&str; 5] = ["pesquis", "procur", "busca ", "no google", "abre o google"];

fn pediu_a_aba(dito: &str) -> bool {
    let normalizado = crate::core::memory::normalizar(dito);
    ORDENS_DE_BUSCA
        .iter()
        .any(|ordem| normalizado.contains(ordem.trim_end()))
}

/// Dá uma olhada na internet, guarda o que achou e responde conversando.
///
/// Nunca devolve `Err`: sem internet, dizer isso na conversa é melhor que derrubar a
/// mensagem inteira com um erro de IPC.
async fn pesquisar_e_responder(
    http: &reqwest::Client,
    settings: &AppSettings,
    memoria: &Memoria,
    consulta: &str,
    log: &mut Log,
) -> String {
    let fonte = search::fonte(&settings.brave_api_key);
    let relogio = Instant::now();

    let achados = match search::pesquisar(http, consulta, &settings.brave_api_key).await {
        Ok(achados) => achados,
        Err(erro) => {
            log.busca(fonte, 0, relogio.elapsed());
            return erro.to_string();
        }
    };
    log.busca(fonte, achados.len(), relogio.elapsed());

    // Guarda ANTES de responder: se o modelo falhar na hora de falar, o conhecimento
    // fica na pasta do mesmo jeito. É isso que faz ele não pesquisar a mesma coisa
    // duas vezes — na próxima, a nota já está no contexto da conversa.
    memoria.aprender(consulta, &converse::nota_da_busca(&achados));
    log.memoria('+', &crate::core::memory::slug(consulta));

    let resposta = converse::responder_com_busca(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &settings.assistant_name,
        consulta,
        &achados,
    )
    .await;

    match resposta {
        Ok(texto) if !texto.is_empty() => texto,
        // Ollama fora do ar: o trecho cru ainda responde melhor que uma mensagem de erro.
        _ => achados[0].trecho.chars().take(400).collect(),
    }
}

/// Destila o que já saiu da janela do prompt, em lotes.
///
/// Silencioso de propósito: falhar em resumir não pode atrapalhar a conversa que
/// acabou de acontecer, e não há o que o usuário faça a respeito.
async fn talvez_resumir(http: &reqwest::Client, settings: &AppSettings, memoria: &Memoria) {
    let pendentes = memoria.pendentes_de_resumo(converse::JANELA, LOTE_DE_RESUMO);
    if pendentes.len() < LOTE_DE_RESUMO {
        return;
    }

    let anterior = memoria
        .notas()
        .into_iter()
        .find(|nota| nota.nome == crate::core::memory::slug(NOTA_DO_RESUMO))
        .map(|nota| nota.corpo)
        .unwrap_or_default();

    let resumido = converse::resumir(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &pendentes,
        &anterior,
    )
    .await;

    if let Ok(texto) = resumido {
        if !texto.is_empty() {
            memoria.guardar_resumo(NOTA_DO_RESUMO, &texto);
            memoria.marcar_resumidas(pendentes.len());
        }
    }
}

/// Título curto para um fato que veio sem assunto (o caminho explícito manda só a
/// frase). As primeiras palavras que carregam significado servem bem.
fn assunto_de(fato: &str) -> String {
    const VAZIAS: [&str; 12] = [
        "que", "de", "do", "da", "em", "no", "na", "um", "uma", "para", "com", "ele",
    ];

    let normalizado = crate::core::memory::normalizar(fato);
    let palavras: Vec<&str> = normalizado
        .split(' ')
        .filter(|palavra| palavra.len() > 2 && !VAZIAS.contains(palavra))
        .take(4)
        .collect();

    if palavras.is_empty() {
        "anotacao".to_owned()
    } else {
        palavras.join(" ")
    }
}

/// Executa e já devolve a frase de confirmação — um `match` só, e a frase pode usar o
/// valor que a ação produziu ("Volume em 60%") em vez de repetir o que foi pedido.
///
/// Frase fixa, e não uma segunda ida ao modelo: é instantânea, determinística, e o
/// TTS tem o que falar sem esperar nada.
fn execute(acao: &Intent) -> Result<String, SystemError> {
    Ok(match acao {
        Intent::OpenSite { url } => {
            system::open_url(url)?;
            format!("Abrindo {url}.")
        }
        Intent::OpenApp { name } => {
            system::open_app(name)?;
            format!("Abrindo o {name}.")
        }
        Intent::VolumeUp { steps } => {
            let nivel = system::nudge_volume(passos(*steps))?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeDown { steps } => {
            let nivel = system::nudge_volume(-passos(*steps))?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeSet { level } => {
            let nivel = (*level).min(100);
            system::set_volume(nivel)?;
            format!("Volume em {nivel}%.")
        }
        Intent::VolumeMute {} => {
            if system::toggle_mute()? {
                "Mudo.".to_owned()
            } else {
                "Som de volta.".to_owned()
            }
        }
        Intent::MediaPlayPause {} => {
            system::press(MediaKey::PlayPause)?;
            "Feito.".to_owned()
        }
        Intent::MediaNext {} => {
            system::press(MediaKey::Next)?;
            "Próxima.".to_owned()
        }
        Intent::MediaPrevious {} => {
            system::press(MediaKey::Previous)?;
            "Anterior.".to_owned()
        }
        // Tratados em `handle`, antes de chegar aqui.
        Intent::Reply {}
        | Intent::WebSearch { .. }
        | Intent::PlayMusic { .. }
        | Intent::WebcamOn {}
        | Intent::WebcamOff {}
        | Intent::Look {}
        | Intent::Remember { .. }
        | Intent::Forget { .. }
        | Intent::Alias { .. } => String::new(),
    })
}

/// Teto em 5 passos: o modelo às vezes inventa um número grande, e ninguém precisa de
/// "aumenta 200".
fn passos(steps: u8) -> i8 {
    i8::try_from(u32::from(steps.clamp(1, 5)) * PASSO_DE_VOLUME as u32).unwrap_or(i8::MAX)
}

/// O log que aparece no chat.
struct Log {
    linhas: Vec<String>,
    /// Sem ação nem mudança de memória, não há log — só houve conversa.
    houve_algo: bool,
}

impl Log {
    fn novo(dito: &str, model: &str, pensou: std::time::Duration) -> Self {
        Self {
            linhas: vec![
                format!("GATILHO    {dito}"),
                format!("INTERPRETE {model} · {:.1} s", pensou.as_secs_f32()),
            ],
            houve_algo: false,
        }
    }

    fn acao(&mut self, acao: &Intent) {
        self.linhas
            .push(format!("AÇÃO       {} · {}", verbo(acao), argumentos(acao)));
        self.houve_algo = true;
    }

    fn resultado(&mut self, resultado: &Result<String, SystemError>, ms: u128) {
        self.desfecho(resultado.as_ref().err().map(ToString::to_string), ms);
    }

    fn desfecho(&mut self, erro: Option<String>, ms: u128) {
        self.linhas.push(match erro {
            None => format!("RESULTADO  ok · {ms} ms"),
            Some(erro) => format!("RESULTADO  falhou · {erro}"),
        });
    }

    fn memoria(&mut self, sinal: char, nome: &str) {
        self.linhas.push(format!("MEMÓRIA    {sinal} {nome}"));
        self.houve_algo = true;
    }

    /// De onde veio a resposta importa: "wikipedia · 0 resultados" explica sozinho por
    /// que o Jarvis disse que não achou nada.
    fn busca(&mut self, fonte: &str, quantos: usize, levou: std::time::Duration) {
        self.linhas.push(format!(
            "BUSCA      {fonte} · {quantos} resultados · {:.1} s",
            levou.as_secs_f32()
        ));
        self.houve_algo = true;
    }

    fn render(self) -> Option<String> {
        self.houve_algo.then(|| self.linhas.join("\n"))
    }
}

fn verbo(acao: &Intent) -> String {
    campos(acao)
        .get("action")
        .and_then(|valor| valor.as_str())
        .unwrap_or("?")
        .to_owned()
}

/// Formata os argumentos reaproveitando a serialização do enum — variante nova
/// aparece no log sem ninguém tocar aqui.
fn argumentos(acao: &Intent) -> String {
    let campos = campos(acao);
    let lista: Vec<String> = campos
        .iter()
        .filter(|(chave, _)| chave.as_str() != "action")
        .map(|(chave, valor)| match valor.as_str() {
            Some(texto) => format!("{chave}={texto}"),
            None => format!("{chave}={valor}"),
        })
        .collect();

    if lista.is_empty() {
        "sem argumentos".to_owned()
    } else {
        lista.join(" · ")
    }
}

fn campos(acao: &Intent) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(acao) {
        Ok(serde_json::Value::Object(mapa)) => mapa,
        _ => serde_json::Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O log tem que dizer o que foi feito COM QUE ALVO — é a única informação que
    /// permite descobrir por que o comando errado disparou.
    #[test]
    fn o_log_mostra_o_verbo_e_o_alvo() {
        let acao = Intent::OpenApp {
            name: "spotify".to_owned(),
        };
        assert_eq!(verbo(&acao), "open_app");
        assert_eq!(argumentos(&acao), "name=spotify");

        // Ação sem argumento não pode virar uma linha vazia no log.
        assert_eq!(verbo(&Intent::MediaNext {}), "media_next");
        assert_eq!(argumentos(&Intent::MediaNext {}), "sem argumentos");
    }

    /// Conversa que não mexeu em nada não gera bolha de log — senão o log vira ruído
    /// embaixo de cada "bom dia" e o usuário para de ler.
    #[test]
    fn conversa_sem_efeito_nao_gera_log() {
        let vazio = Log::novo(
            "bom dia",
            "qwen2.5:3b",
            std::time::Duration::from_millis(400),
        );
        assert!(vazio.render().is_none());

        let mut com_memoria = Log::novo(
            "meu gato chama Bidu",
            "qwen2.5:3b",
            std::time::Duration::from_millis(900),
        );
        com_memoria.memoria('+', "gato bidu");

        let texto = com_memoria.render().expect("tem que gerar log");
        assert!(texto.contains("GATILHO    meu gato chama Bidu"));
        assert!(texto.contains("MEMÓRIA    + gato bidu"));
    }

    /// "quem foi santos dumont?" no meio de um papo não pode abrir uma aba do Google
    /// na cara do usuário; "pesquisa isso aí" pode.
    #[test]
    fn so_abre_a_aba_quando_mandaram_pesquisar() {
        for ordem in [
            "pesquisa preço do dólar",
            "procura a receita de pão de queijo",
            "quem foi tesla, pesquisa aí",
            "busca isso no google",
            "abre o google e vê quem descobriu o brasil",
        ] {
            assert!(pediu_a_aba(ordem), "devia abrir a aba: {ordem:?}");
        }

        for papo in [
            "quem foi santos dumont?",
            "o que é uma black hole",
            "qual a capital da austrália",
            "como faz pão de queijo",
        ] {
            assert!(!pediu_a_aba(papo), "não devia abrir a aba: {papo:?}");
        }
    }

    /// O modelo alucina número grande, e "aumenta 200" não pode estourar o `i8` nem
    /// virar um volume absurdo.
    #[test]
    fn passos_ficam_no_teto() {
        assert_eq!(passos(0), PASSO_DE_VOLUME);
        assert_eq!(passos(1), PASSO_DE_VOLUME);
        assert_eq!(passos(3), PASSO_DE_VOLUME * 3);
        assert_eq!(passos(200), PASSO_DE_VOLUME * 5);
    }

    /// O caminho explícito ("lembra que...") manda só a frase, sem título. Um assunto
    /// ruim aqui vira um arquivo com nome ruim na pasta que o usuário vai abrir.
    #[test]
    fn deriva_um_assunto_legivel_do_fato() {
        assert_eq!(
            assunto_de("Acorda 6h30 para a academia."),
            "acorda 6h30 academia"
        );
        assert_eq!(
            assunto_de("Tem um gato chamado Bidu."),
            "tem gato chamado bidu"
        );
        assert_eq!(assunto_de("de um"), "anotacao");
    }
}
