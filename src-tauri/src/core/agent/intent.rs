//! O que o modelo entendeu, e como perguntar a ele.
//!
//! Uma chamada só ao Ollama, com o JSON Schema no campo `format` — não é um loop de
//! tool use. A tarefa aqui é classificação: uma frase entra, um verbo e seus
//! argumentos saem. Um modelo de 3B faz isso bem; o mesmo modelo num loop de
//! múltiplos passos, não.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::AgentError;

/// O que o modelo pode pedir.
///
/// `#[serde(tag = "action")]` faz o JSON sair PLANO — `{"action":"open_site","url":…}`
/// — que é exatamente a forma que [`schema`] descreve. Uma variante por verbo, sem
/// aninhar volume e mídia em sub-enums: aninhamento vira `oneOf` no schema, e a
/// grammar que o llama.cpp gera a partir dele fica bem menos confiável num 3B.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Intent {
    OpenSite {
        url: String,
    },
    OpenApp {
        name: String,
    },
    VolumeUp {
        #[serde(default = "um_passo")]
        steps: u8,
    },
    VolumeDown {
        #[serde(default = "um_passo")]
        steps: u8,
    },
    VolumeSet {
        level: u8,
    },
    /// Chaves vazias, e NÃO uma variante unitária: o schema é frouxo e o modelo às
    /// vezes manda `{"action":"volume_mute","steps":0}`. Struct vazia ignora o
    /// acompanhante; variante unitária recusaria o mapa inteiro.
    VolumeMute {},
    MediaPlayPause {},
    MediaNext {},
    MediaPrevious {},
    WebSearch {
        query: String,
    },
    /// Nada a executar: conversa fiada OU pedido que não bate com nenhuma capacidade.
    /// Uma variante só para os dois casos porque o desfecho é idêntico — falar e não
    /// agir.
    Reply {
        text: String,
    },
}

fn um_passo() -> u8 {
    1
}

/// Fonte única da lista de verbos: alimenta o schema, e o teste quebra se algum dia
/// ela divergir do enum.
const ACOES: [&str; 11] = [
    "open_site",
    "open_app",
    "volume_up",
    "volume_down",
    "volume_set",
    "volume_mute",
    "media_play_pause",
    "media_next",
    "media_previous",
    "web_search",
    "reply",
];

/// O schema é frouxo DE PROPÓSITO: objeto plano, todos os campos opcionais menos o
/// verbo. Ele garante a FORMA (é um objeto, e o verbo está na lista); quem valida a
/// combinação verbo↔campos é o serde, no `from_str` lá embaixo — `open_site` sem
/// `url` falha o parse e vira [`AgentError::NaoEntendi`].
///
/// A alternativa exata seria um `oneOf` de 11 objetos. Seria a verdade completa, e um
/// pesadelo para a grammar do llama.cpp.
pub fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": ACOES },
            "url":    { "type": "string" },
            "name":   { "type": "string" },
            "query":  { "type": "string" },
            "text":   { "type": "string" },
            "steps":  { "type": "integer" },
            "level":  { "type": "integer" }
        },
        "required": ["action"]
    })
}

/// Carregar o modelo na VRAM na primeira chamada leva mais de um minuto e meio nesta
/// classe de máquina — medido. Depois de quente ele responde em ~0,5 s. O timeout
/// precisa caber o pior caso, senão o primeiro comando do dia sempre falha.
const TIMEOUT: Duration = Duration::from_secs(180);

/// Quanto tempo o Ollama mantém o modelo na memória depois da última chamada. O
/// padrão dele é 5 minutos, e pagar 90 s de recarga porque o usuário foi almoçar é
/// justamente o que estraga a experiência.
const KEEP_ALIVE: &str = "2h";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .unwrap_or_default()
}

/// Manda a frase ao Ollama e devolve a ação.
pub async fn interpret(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    assistant_name: &str,
    frase: &str,
) -> Result<Intent, AgentError> {
    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": KEEP_ALIVE,
        "format": schema(),
        // Isto é classificação, não redação: temperatura 0 e teto curto de saída.
        "options": { "temperature": 0, "num_predict": 200 },
        "messages": [
            { "role": "system", "content": system_prompt(assistant_name) },
            { "role": "user", "content": frase },
        ],
    });

    let endpoint = format!("{}/api/chat", url.trim_end_matches('/'));
    let resposta = http
        .post(&endpoint)
        .json(&corpo)
        .send()
        .await
        .map_err(|error| rede(error, url, model))?;

    let status = resposta.status();
    if !status.is_success() {
        let corpo = resposta.text().await.unwrap_or_default();
        // 404 é como o Ollama diz "não tenho esse modelo baixado".
        return Err(if status == reqwest::StatusCode::NOT_FOUND {
            AgentError::ModeloAusente(model.to_owned())
        } else {
            AgentError::Recusado {
                status: status.as_u16(),
                corpo: corpo.chars().take(300).collect(),
            }
        });
    }

    #[derive(Deserialize)]
    struct Envelope {
        message: Mensagem,
    }
    #[derive(Deserialize)]
    struct Mensagem {
        content: String,
    }

    // Dois parses: o JSON da API traz o JSON da ação como STRING dentro de `content`.
    let envelope: Envelope = resposta
        .json()
        .await
        .map_err(|error| rede(error, url, model))?;

    serde_json::from_str(envelope.message.content.trim())
        .map_err(|error| AgentError::NaoEntendi(format!("{error} — {}", envelope.message.content)))
}

fn rede(error: reqwest::Error, url: &str, model: &str) -> AgentError {
    if error.is_connect() {
        return AgentError::Offline {
            url: url.to_owned(),
            model: model.to_owned(),
        };
    }
    if error.is_timeout() {
        return AgentError::Demorou;
    }
    AgentError::Rede(error.to_string())
}

/// Curto e em português, com a tabela de verbos e exemplos.
///
/// Os exemplos não são enfeite: num modelo de 3B eles valem mais que a descrição. Os
/// de mídia estão aí porque sem eles "pula essa música" e "volta pra anterior" caíam
/// em `media_play_pause` — medido.
fn system_prompt(assistant_name: &str) -> String {
    format!(
        "Você é o roteador de comandos do {assistant_name}, um assistente de desktop Windows.
Leia a frase do usuário e devolva UMA ação em JSON. Nada de texto fora do JSON.

open_site         abrir um site. `url` = endereço completo com https://.
open_app          abrir um programa instalado. `name` = só o nome, sem caminho (ex.: spotify, notepad).
volume_up         aumentar o volume. `steps` = quantos passos (1 se não disser).
volume_down       diminuir o volume. Mesma regra.
volume_set        volume em valor absoluto. `level` = 0 a 100.
volume_mute       mudo, ou tirar do mudo.
media_play_pause  pausar ou retomar o que está tocando.
media_next        pular para a PRÓXIMA música/faixa.
media_previous    voltar para a música/faixa ANTERIOR.
web_search        pesquisar no Google. `query` = só os termos, sem \"pesquise\" nem \"no google\".
reply             TUDO o mais: conversa, pergunta, ou pedido que não cabe nas ações acima.
                  `text` = a sua resposta, em português, curta.

Nunca invente uma ação, e nunca invente termos que o usuário não disse.
Na dúvida, use reply.

Exemplos:
\"abre o youtube\"                    -> {{\"action\":\"open_site\",\"url\":\"https://www.youtube.com\"}}
\"põe o spotify pra rodar\"           -> {{\"action\":\"open_app\",\"name\":\"spotify\"}}
\"abaixa dois\"                       -> {{\"action\":\"volume_down\",\"steps\":2}}
\"deixa em 30\"                       -> {{\"action\":\"volume_set\",\"level\":30}}
\"pausa\"                             -> {{\"action\":\"media_play_pause\"}}
\"pula essa música\"                  -> {{\"action\":\"media_next\"}}
\"próxima faixa\"                     -> {{\"action\":\"media_next\"}}
\"volta pra anterior\"                -> {{\"action\":\"media_previous\"}}
\"pesquisa no google quem foi tesla\" -> {{\"action\":\"web_search\",\"query\":\"quem foi tesla\"}}
\"quem descobriu o brasil\"           -> {{\"action\":\"reply\",\"text\":\"Pedro Álvares Cabral, em 1500.\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O schema que vai para o Ollama e o enum que o serde parseia são a mesma
    /// verdade. Este teste quebra se: variante nova sem entrada em `ACOES`, `rename`
    /// trocado, `default` sumido, ou tag renomeada.
    #[test]
    fn o_schema_e_o_enum_falam_a_mesma_lingua() {
        let amostras = [
            (
                r#"{"action":"open_site","url":"https://www.youtube.com"}"#,
                Intent::OpenSite {
                    url: "https://www.youtube.com".to_owned(),
                },
            ),
            (
                r#"{"action":"open_app","name":"spotify"}"#,
                Intent::OpenApp {
                    name: "spotify".to_owned(),
                },
            ),
            (
                r#"{"action":"volume_up","steps":3}"#,
                Intent::VolumeUp { steps: 3 },
            ),
            // Sem `steps`: é o default de 1 que faz "aumenta o volume" funcionar.
            (
                r#"{"action":"volume_down"}"#,
                Intent::VolumeDown { steps: 1 },
            ),
            (
                r#"{"action":"volume_set","level":30}"#,
                Intent::VolumeSet { level: 30 },
            ),
            // Campo estranho junto: o schema é frouxo e o modelo emite isso de
            // verdade (`{"action":"volume_mute","steps":0}` apareceu no teste real).
            (
                r#"{"action":"volume_mute","steps":0}"#,
                Intent::VolumeMute {},
            ),
            (
                r#"{"action":"media_play_pause"}"#,
                Intent::MediaPlayPause {},
            ),
            (r#"{"action":"media_next"}"#, Intent::MediaNext {}),
            (r#"{"action":"media_previous"}"#, Intent::MediaPrevious {}),
            (
                r#"{"action":"web_search","query":"preço do dólar"}"#,
                Intent::WebSearch {
                    query: "preço do dólar".to_owned(),
                },
            ),
            (
                r#"{"action":"reply","text":"tudo certo por aqui"}"#,
                Intent::Reply {
                    text: "tudo certo por aqui".to_owned(),
                },
            ),
        ];

        let schema = schema();
        let acoes = schema["properties"]["action"]["enum"]
            .as_array()
            .expect("o schema precisa listar as ações");

        assert_eq!(
            acoes.len(),
            amostras.len(),
            "variante sem amostra, ou ação no schema que o enum não conhece"
        );

        for (json, esperado) in amostras {
            let intent: Intent = serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("não parseou {json}: {error}"));
            assert_eq!(intent, esperado);

            let verbo = serde_json::to_value(&intent).expect("serializa")["action"].clone();
            assert!(acoes.contains(&verbo), "{verbo} não está no schema");
        }

        // O portão de verdade é o serde, não o schema: campo obrigatório faltando
        // TEM que falhar, para virar `NaoEntendi` em vez de uma ação sem alvo.
        assert!(serde_json::from_str::<Intent>(r#"{"action":"open_site"}"#).is_err());
        assert!(serde_json::from_str::<Intent>(r#"{"action":"voar"}"#).is_err());
    }
}
