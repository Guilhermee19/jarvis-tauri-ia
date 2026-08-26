//! O agente: entende a frase e executa.
//!
//! Substitui o `core::chat::mock_reply` da v0.1. O ciclo é curto de propósito —
//! **uma** chamada ao intérprete local ([`intent`]), **uma** ação em [`crate::core::system`],
//! e uma frase de volta. Não é o loop de tool use da Anthropic; quando ele entrar,
//! entra ao lado, atrás desta mesma função [`handle`].
//!
//! O que sai daqui são duas coisas: a resposta que o usuário lê, e o LOG do gatilho —
//! o que foi ouvido, o que o modelo entendeu e no que deu. O log existe porque um
//! assistente que abre programas erra em silêncio se ninguém puder auditar o que ele
//! achou que foi pedido.

mod intent;

use std::time::Instant;

pub use intent::client;

use intent::Intent;

use crate::config::AppSettings;
use crate::core::system::{self, MediaKey, SystemError};

/// Um passo de volume. A tecla do Windows anda ~2%, o que é imperceptível quando
/// alguém fala "aumenta o volume" — em comando de voz um passo precisa ser um passo.
const PASSO_DE_VOLUME: i8 = 10;

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
}

/// O que [`handle`] devolve: a fala, e o log quando houve comando.
pub struct Outcome {
    /// `None` em conversa fiada. Uma caixa de log embaixo de cada "bom dia" é o
    /// ruído que faz o log inteiro passar a ser ignorado.
    pub trace: Option<String>,
    pub reply: String,
}

pub async fn handle(
    http: &reqwest::Client,
    settings: &AppSettings,
    dito: &str,
) -> Result<Outcome, AgentError> {
    // Modelo vazio desliga o intérprete e volta ao mock. É a saída de emergência sem
    // precisar de um booleano — mesmo padrão do `tts_voice_id` ("vazio = padrão").
    if settings.ollama_model.trim().is_empty() {
        return Ok(Outcome {
            trace: None,
            reply: crate::core::chat::mock_reply_text(&settings.assistant_name, dito),
        });
    }

    let relogio = Instant::now();
    let acao = intent::interpret(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        &settings.assistant_name,
        dito,
    )
    .await?;
    let pensou = relogio.elapsed();

    if let Intent::Reply { text } = acao {
        return Ok(Outcome {
            trace: None,
            reply: text,
        });
    }

    let relogio = Instant::now();
    let resultado = execute(&acao);
    let levou = relogio.elapsed();

    let desfecho = match &resultado {
        Ok(_) => format!("ok · {} ms", levou.as_millis()),
        Err(erro) => format!("falhou · {erro}"),
    };

    Ok(Outcome {
        trace: Some(format!(
            "GATILHO    {dito}\n\
             INTERPRETE {} · {:.1} s\n\
             AÇÃO       {} · {}\n\
             RESULTADO  {desfecho}",
            settings.ollama_model,
            pensou.as_secs_f32(),
            verbo(&acao),
            argumentos(&acao),
        )),
        // Falha de execução responde como FRASE, não como erro de IPC: "não achei o
        // Spotify" é uma conversa, não o backend caindo.
        reply: match resultado {
            Ok(frase) => frase,
            Err(erro) => erro.to_string(),
        },
    })
}

/// Executa e já devolve a frase de confirmação — um `match` só, e a frase pode usar o
/// valor que a ação produziu ("Volume em 60%") em vez de repetir o que foi pedido.
///
/// Frase fixa, e não uma segunda ida ao modelo: é instantânea, determinística, e o
/// TTS tem o que falar sem esperar nada.
fn execute(acao: &Intent) -> Result<String, SystemError> {
    Ok(match acao {
        Intent::OpenSite { url } => {
            let alvo = system::open_url(url).map(|()| url)?;
            format!("Abrindo {alvo}.")
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
        Intent::WebSearch { query } => {
            system::search_web(query)?;
            format!("Pesquisando \"{query}\".")
        }
        // Tratado em `handle`, antes de chegar aqui.
        Intent::Reply { text } => text.clone(),
    })
}

/// Teto em 5 passos: o modelo às vezes inventa um número grande, e ninguém precisa de
/// "aumenta 200".
fn passos(steps: u8) -> i8 {
    i8::try_from(u32::from(steps.clamp(1, 5)) * PASSO_DE_VOLUME as u32).unwrap_or(i8::MAX)
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

    /// O modelo alucina número grande, e "aumenta 200" não pode estourar o `i8` nem
    /// virar um volume absurdo.
    #[test]
    fn passos_ficam_no_teto() {
        assert_eq!(passos(0), PASSO_DE_VOLUME);
        assert_eq!(passos(1), PASSO_DE_VOLUME);
        assert_eq!(passos(3), PASSO_DE_VOLUME * 3);
        assert_eq!(passos(200), PASSO_DE_VOLUME * 5);
    }
}
