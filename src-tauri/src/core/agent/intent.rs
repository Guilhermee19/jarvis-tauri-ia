//! O que o modelo entendeu, e como perguntar a ele.
//!
//! Uma chamada só ao Ollama, com o JSON Schema no campo `format` — não é um loop de
//! tool use. A tarefa aqui é classificação: uma frase entra, um verbo e seus
//! argumentos saem. Um modelo de 3B faz isso bem; o mesmo modelo num loop de
//! múltiplos passos, não.
//!
//! É aqui também que a memória fecha o laço de aprendizado: os apelidos que o usuário
//! já ensinou entram no system prompt, então "abre meu jogo" passa a funcionar depois
//! de ensinado UMA vez — sem treinar nada, sem tocar em peso nenhum.

use std::collections::BTreeMap;
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
    /// "toque Charlie Brown Jr só os loucos sabem no spotify". Diferente de
    /// [`Intent::OpenApp`], que só abre o programa, e de [`Intent::MediaPlayPause`],
    /// que retoma o que já estava tocando.
    PlayMusic {
        query: String,
    },
    /// Liga a câmera na tela, exatamente como o botão da barra de ícones. NÃO é
    /// [`Intent::OpenApp`] com "camera": o dono do preview é a UI, e abrir o
    /// dispositivo pelo Rust deixaria o botão apagado com a câmera ligada.
    WebcamOn {},
    WebcamOff {},
    /// "o que é isso?", "olha isso aqui". Liga a câmera, tira um quadro e conta o que
    /// vê. Diferente de [`Intent::WebcamOn`], que só liga e mostra.
    Look {},
    /// "lembra que eu acordo 6h30". O caminho EXPLÍCITO da memória, e o confiável — a
    /// extração automática em `converse` é best-effort.
    Remember {
        fact: String,
    },
    /// "esquece a academia".
    Forget {
        about: String,
    },
    /// "meu jogo é o steam", "quando eu falar trabalho abre o code". O que faz o
    /// roteador melhorar com o uso.
    Alias {
        nickname: String,
        target: String,
    },
    /// Nada a executar: conversa fiada OU pedido que não bate com nenhuma capacidade.
    /// Quem responde isso é `converse`, com histórico e memória — não este prompt.
    Reply {},
}

fn um_passo() -> u8 {
    1
}

/// Fonte única da lista de verbos: alimenta o schema, e o teste quebra se algum dia
/// ela divergir do enum.
const ACOES: [&str; 18] = [
    "play_music",
    "webcam_on",
    "webcam_off",
    "look",
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
    "remember",
    "forget",
    "alias",
    "reply",
];

/// O schema é frouxo DE PROPÓSITO: objeto plano, todos os campos opcionais menos o
/// verbo. Ele garante a FORMA (é um objeto, e o verbo está na lista); quem valida a
/// combinação verbo↔campos é o serde, no `from_str` lá embaixo — `open_site` sem
/// `url` falha o parse e vira [`AgentError::NaoEntendi`].
///
/// A alternativa exata seria um `oneOf` de 14 objetos. Seria a verdade completa, e um
/// pesadelo para a grammar.
pub fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action":   { "type": "string", "enum": ACOES },
            "url":      { "type": "string" },
            "name":     { "type": "string" },
            "query":    { "type": "string" },
            "fact":     { "type": "string" },
            "about":    { "type": "string" },
            "nickname": { "type": "string" },
            "target":   { "type": "string" },
            "steps":    { "type": "integer" },
            "level":    { "type": "integer" }
        },
        "required": ["action"]
    })
}

/// Carregar o modelo na VRAM na primeira chamada leva mais de um minuto e meio nesta
/// classe de máquina — medido. Depois de quente ele responde em ~0,4 s. O timeout
/// precisa caber o pior caso, senão o primeiro comando do dia sempre falha.
const TIMEOUT: Duration = Duration::from_secs(180);

/// Quanto tempo o Ollama mantém o modelo na memória depois da última chamada. O padrão
/// dele é 5 minutos, e pagar 90 s de recarga porque o usuário foi almoçar é justamente
/// o que estraga a experiência.
pub(crate) const KEEP_ALIVE: &str = "2h";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .unwrap_or_default()
}

/// POST cru ao `/api/chat`, devolvendo o `message.content`. Compartilhado com
/// `converse`, para os erros do Ollama serem traduzidos num lugar só.
pub(crate) async fn pedir(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    corpo: &serde_json::Value,
) -> Result<String, AgentError> {
    let endpoint = format!("{}/api/chat", url.trim_end_matches('/'));
    let resposta = http
        .post(&endpoint)
        .json(corpo)
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

    let envelope: Envelope = resposta
        .json()
        .await
        .map_err(|error| rede(error, url, model))?;

    Ok(envelope.message.content)
}

/// Manda a frase ao Ollama e devolve a ação.
pub async fn interpret(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    assistant_name: &str,
    apelidos: &BTreeMap<String, String>,
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
            { "role": "system", "content": system_prompt(assistant_name, apelidos) },
            { "role": "user", "content": frase },
        ],
    });

    // Dois parses: o JSON da API traz o JSON da ação como STRING dentro de `content`.
    let texto = pedir(http, url, model, &corpo).await?;
    serde_json::from_str(texto.trim())
        .map_err(|erro| AgentError::NaoEntendi(format!("{erro} — {texto}")))
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
fn system_prompt(assistant_name: &str, apelidos: &BTreeMap<String, String>) -> String {
    let mut prompt = format!(
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
play_music        TOCAR uma música específica que ele nomeou. `query` = artista e nome
                  da música, sem \"toca\", sem \"põe\" e sem \"no spotify\".
webcam_on         ligar a câmera na tela.
webcam_off        desligar a câmera.
look              OLHAR pela câmera e dizer o que está vendo. É quando ele aponta algo
                  para a webcam e pergunta o que é.
web_search        pesquisar sobre o MUNDO. `query` = só os termos, sem \"pesquise\" nem \"no google\".
remember          ele MANDOU guardar algo. `fact` = o que guardar, em terceira pessoa.
forget            ele mandou esquecer algo. `about` = o assunto a apagar.
alias             ele ensinou um apelido. `nickname` = o apelido, `target` = o programa ou site.
reply             conversa, papo, desabafo, e perguntas sobre ELE. Sem argumento nenhum.

A REGRA MAIS IMPORTANTE: comando é ORDEM CURTA E DIRETA. Se a frase conta algo,
desabafa, opina, reclama, agradece ou só puxa assunto, é SEMPRE reply — mesmo que ela
mencione música, volume, um site ou um programa. MENCIONAR NÃO É MANDAR.

Na dúvida entre um comando e reply, escolha reply. Executar sem ser pedido é o pior
erro que você pode cometer.

Perguntas se dividem em duas: sobre o MUNDO (fatos, pessoas, coisas, notícias) vai para
web_search; sobre ELE ou sobre vocês dois vai para reply, porque a resposta está na
memória e não na internet.

\"abre o X\" também se divide em duas, e errar aqui não abre nada:
- X é um SITE ou serviço da web (youtube, gmail, netflix, globo, chatgpt, instagram)
  -> open_site, com a URL completa.
- X é um PROGRAMA instalado no PC (spotify, notepad, calculadora, steam, discord)
  -> open_app, só o nome.

Nunca invente uma ação, e nunca invente termos que o usuário não disse.

Exemplos de COMANDO:
\"abre o youtube\"                    -> {{\"action\":\"open_site\",\"url\":\"https://www.youtube.com\"}}
\"põe o spotify pra rodar\"           -> {{\"action\":\"open_app\",\"name\":\"spotify\"}}
\"abaixa dois\"                       -> {{\"action\":\"volume_down\",\"steps\":2}}
\"deixa em 30\"                       -> {{\"action\":\"volume_set\",\"level\":30}}
\"pausa\"                             -> {{\"action\":\"media_play_pause\"}}
\"pula essa música\"                  -> {{\"action\":\"media_next\"}}
\"volta pra anterior\"                -> {{\"action\":\"media_previous\"}}
\"toque Charlie Brown Jr só os loucos sabem no spotify\" -> {{\"action\":\"play_music\",\"query\":\"Charlie Brown Jr Só os Loucos Sabem\"}}
\"põe Bohemian Rhapsody pra tocar\"   -> {{\"action\":\"play_music\",\"query\":\"Queen Bohemian Rhapsody\"}}
\"coloca uma música do Djavan\"       -> {{\"action\":\"play_music\",\"query\":\"Djavan\"}}
\"abre o spotify\"                    -> {{\"action\":\"open_app\",\"name\":\"spotify\"}}
\"abre a webcam\"                     -> {{\"action\":\"webcam_on\"}}
\"liga a câmera\"                     -> {{\"action\":\"webcam_on\"}}
\"desliga a câmera\"                  -> {{\"action\":\"webcam_off\"}}
\"fecha a webcam\"                    -> {{\"action\":\"webcam_off\"}}
\"o que é isso?\"                     -> {{\"action\":\"look\"}}
\"olha isso aqui\"                    -> {{\"action\":\"look\"}}
\"o que você está vendo?\"            -> {{\"action\":\"look\"}}
\"que objeto é esse na minha mão\"    -> {{\"action\":\"look\"}}
\"lembra que eu acordo 6h30\"         -> {{\"action\":\"remember\",\"fact\":\"Acorda 6h30.\"}}
\"esquece a academia\"                -> {{\"action\":\"forget\",\"about\":\"academia\"}}
\"meu jogo é o steam\"                -> {{\"action\":\"alias\",\"nickname\":\"meu jogo\",\"target\":\"steam\"}}

Exemplos de PERGUNTA SOBRE O MUNDO — vão para web_search:
\"pesquisa no google quem foi tesla\" -> {{\"action\":\"web_search\",\"query\":\"nikola tesla\"}}
\"quem descobriu o brasil?\"          -> {{\"action\":\"web_search\",\"query\":\"descobrimento do brasil\"}}
\"o que é rust?\"                     -> {{\"action\":\"web_search\",\"query\":\"rust linguagem de programação\"}}
\"como faz pão de queijo\"            -> {{\"action\":\"web_search\",\"query\":\"receita de pão de queijo\"}}

Exemplos de CONVERSA — todos reply, mesmo citando música, jogo ou programa:
\"po enquanto nada, quero é ir pra casa pra poder jogar\" -> {{\"action\":\"reply\"}}
\"não pedi nada pra vc, to apenas conversando\"           -> {{\"action\":\"reply\"}}
\"essa música que tocou agora é boa demais\"              -> {{\"action\":\"reply\"}}
\"o volume do meu fone tá estourando os ouvidos\"         -> {{\"action\":\"reply\"}}
\"passei o dia todo no vscode\"                           -> {{\"action\":\"reply\"}}
\"acho o youtube viciante demais\"                        -> {{\"action\":\"reply\"}}
\"que horas eu acordo mesmo?\"                            -> {{\"action\":\"reply\"}}
\"bom dia\"                                               -> {{\"action\":\"reply\"}}
\"e aí, tudo certo?\"                                     -> {{\"action\":\"reply\"}}"
    );

    // O laço de aprendizado. Sem isto, "abre meu jogo" seria um `open_app` com
    // `name: "meu jogo"`, que o Windows não resolve — e nenhuma quantidade de
    // exemplos genéricos consertaria, porque o apelido é dele.
    if !apelidos.is_empty() {
        let lista: Vec<String> = apelidos
            .iter()
            .map(|(apelido, alvo)| format!("\"{apelido}\" = {alvo}"))
            .collect();

        prompt.push_str(&format!(
            "\n\nAPELIDOS QUE ELE JÁ ENSINOU — troque pelo alvo antes de responder:\n{}",
            lista.join("\n")
        ));
    }

    prompt
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
                r#"{"action":"play_music","query":"Charlie Brown Jr Só os Loucos Sabem"}"#,
                Intent::PlayMusic {
                    query: "Charlie Brown Jr Só os Loucos Sabem".to_owned(),
                },
            ),
            (
                r#"{"action":"web_search","query":"preço do dólar"}"#,
                Intent::WebSearch {
                    query: "preço do dólar".to_owned(),
                },
            ),
            (
                r#"{"action":"remember","fact":"Acorda 6h30."}"#,
                Intent::Remember {
                    fact: "Acorda 6h30.".to_owned(),
                },
            ),
            (
                r#"{"action":"forget","about":"academia"}"#,
                Intent::Forget {
                    about: "academia".to_owned(),
                },
            ),
            (
                r#"{"action":"alias","nickname":"meu jogo","target":"steam"}"#,
                Intent::Alias {
                    nickname: "meu jogo".to_owned(),
                    target: "steam".to_owned(),
                },
            ),
            (r#"{"action":"webcam_on"}"#, Intent::WebcamOn {}),
            (r#"{"action":"webcam_off"}"#, Intent::WebcamOff {}),
            (r#"{"action":"look"}"#, Intent::Look {}),
            (r#"{"action":"reply"}"#, Intent::Reply {}),
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
        assert!(serde_json::from_str::<Intent>(r#"{"action":"alias","nickname":"x"}"#).is_err());
        assert!(serde_json::from_str::<Intent>(r#"{"action":"voar"}"#).is_err());
    }

    /// O laço de aprendizado: sem os apelidos no prompt, o roteador não tem como saber
    /// que "meu jogo" quer dizer steam, e nenhum exemplo genérico resolveria.
    #[test]
    fn os_apelidos_aprendidos_entram_no_prompt() {
        let vazio = system_prompt("Jarvis", &BTreeMap::new());
        assert!(!vazio.contains("APELIDOS"), "sem apelido não gasta prompt");

        let apelidos = BTreeMap::from([("meu jogo".to_owned(), "steam".to_owned())]);
        let com = system_prompt("Jarvis", &apelidos);

        assert!(com.contains("APELIDOS QUE ELE JÁ ENSINOU"));
        assert!(com.contains("\"meu jogo\" = steam"));
    }
}
