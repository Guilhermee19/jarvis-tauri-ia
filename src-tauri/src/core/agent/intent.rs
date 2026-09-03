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
use crate::core::cameras;
use crate::core::vision;

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
    /// "como está o tempo?", "vai chover hoje?" — onde ele ESTÁ.
    ///
    /// Chaves vazias, como o [`Intent::VolumeMute`]: sem campo nenhum para preencher, não
    /// há campo para alucinar.
    Weather {},
    /// "como está o tempo em Lisboa?" — numa cidade que ele nomeou.
    ///
    /// **Verbo separado, e não um campo opcional no [`Intent::Weather`].** Foi medido
    /// contra o modelo de 3B: com um `local` opcional, ele preenchia a cidade em TODA
    /// pergunta, inclusive nas que não citavam lugar nenhum — inventando "São Paulo" para
    /// "vai chover hoje?". A causa é o schema: um campo declarado é um campo que a
    /// gramática deixa emitir, e o modelo prefere emitir a omitir.
    ///
    /// É a mesma lição que separou `smart_home`/`smart_color`/`smart_bright`: mais
    /// entradas na lista de verbos, menos campos por entrada.
    WeatherAt {
        local: String,
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
    /// "que mouse é esse?", "o que tem na minha tela?". Tira uma imagem — da câmera ou
    /// da tela — e responde a pergunta olhando para ela. Diferente de
    /// [`Intent::WebcamOn`], que só liga e mostra.
    ///
    /// A PERGUNTA não é campo daqui. Ela já chega inteira em `handle` como o texto do
    /// usuário, e pedir ao modelo para extraí-la seria mais um campo para ele errar num
    /// prompt que o README já chama de a peça mais frágil do app. `fonte` é o mínimo
    /// que só o roteador consegue decidir.
    Look {
        #[serde(default = "onde_der")]
        fonte: vision::Fonte,
    },
    /// "mostra a garagem", "abre a câmera do portão". Uma câmera de SEGURANÇA da casa,
    /// que tem nome e fica numa janela própria.
    ///
    /// **Verbo separado do [`Intent::WebcamOn`], e não um campo `qual` nele.** São dois
    /// aparelhos diferentes com um nome só em português: a webcam do computador não tem
    /// nome, e a câmera da garagem só existe pelo nome. Juntá-los num verbo faria "liga
    /// a câmera" ter que escolher entre eles por um campo opcional — que é exatamente o
    /// desenho que falhou em `weather`/`weather_at` e em `smart_home`.
    ///
    /// Como o preview é da UI (mesma razão do `webcam_on`), isto vira uma
    /// [`super::AcaoDeUi`].
    CameraOn {
        /// O nome que ele falou. Quem casa "garagem" com a câmera certa é o catálogo,
        /// que conhece os nomes de verdade e este prompt não. Pode vir vazio quando ele
        /// só disse "mostra a câmera" — com uma câmera só cadastrada, isso basta.
        #[serde(default)]
        camera: String,
    },
    /// "fecha as câmeras". Chaves vazias, como o [`Intent::VolumeMute`].
    CameraOff {},
    /// "tem alguém na garagem?", "o carro está na frente?". Olha a câmera de segurança e
    /// responde — o [`Intent::Look`] da câmera de rede.
    ///
    /// Separado do `look` pelo mesmo motivo do `camera_on`: `fonte` é um enum fechado de
    /// lugares fixos (tela, webcam), e uma câmera de segurança é um NOME, que não cabe
    /// naquele enum sem transformá-lo em texto livre.
    LookCamera {
        #[serde(default)]
        camera: String,
    },
    /// "vira a câmera pra esquerda", "olha mais pra cima".
    ///
    /// **Um verbo com `direcao`, e não quatro verbos**, ao contrário de
    /// `volume_up`/`volume_down` — e a diferença é o schema: `direcao` é um enum FECHADO
    /// de quatro valores, então a gramática não deixa o modelo inventar. O que forçou
    /// verbos separados nos outros casos foi campo de texto livre, que não é este caso.
    CameraMove {
        #[serde(default)]
        camera: String,
        direcao: cameras::onvif::Direcao,
    },
    /// "lembra que eu acordo 6h30". O caminho EXPLÍCITO da memória, e o confiável — a
    /// extração automática em `converse` é best-effort.
    Remember {
        fact: String,
    },
    /// "esquece a academia".
    Forget {
        about: String,
    },
    /// "eu sou o Guilherme", "sou a Ana", "meu nome é Bruno".
    ///
    /// Guarda o ROSTO de quem está na webcam sob esse nome, para o Jarvis saudar pelo
    /// nome na próxima abertura. É a resposta natural ao "não te reconheci, quem é você?"
    /// que ele faz quando vê alguém desconhecido — mas funciona a qualquer momento, e é
    /// por isso que é um verbo e não um modo de conversa: um estado "esperando o nome"
    /// prenderia a próxima frase mesmo quando ela fosse sobre outra coisa.
    ///
    /// **Campo `pessoa`, e não `name`.** O `name` do [`Intent::OpenApp`] quer dizer "nome
    /// de um programa"; um campo com dois significados é o tipo de ambiguidade que um 3B
    /// resolve errado — a mesma lição que separou `aparelho` de `target`.
    SouEu {
        pessoa: String,
    },
    /// "meu jogo é o steam", "quando eu falar trabalho abre o code". O que faz o
    /// roteador melhorar com o uso.
    Alias {
        nickname: String,
        target: String,
    },
    /// "apaga a luz da cozinha", "deixa a lâmpada da mesa azul", "põe a luz em 30%".
    ///
    /// `aparelho` é o NOME que ele deu no app da casa inteligente, do jeito que ele
    /// falou — quem casa "luz da cozinha" com "Luz Cozinha" é o chaveiro, que conhece os
    /// nomes de verdade e este prompt não.
    ///
    /// Campo próprio em vez de reaproveitar o `target` do [`Intent::Alias`]: os dois
    /// significam coisas diferentes, e um nome de campo que quer dizer duas coisas é
    /// exatamente o tipo de ambiguidade que um modelo de 3B resolve errado.
    ///
    /// **Um verbo por gesto, e não um verbo com um campo de modo.**
    ///
    /// A primeira versão disto era `smart_home` com um campo `acao` valendo "ligar",
    /// "cor" ou "brilho". Medido contra o modelo de 3B, ele **nunca preencheu esse
    /// campo**: devolvia `{"action":"smart_home","aparelho":"lâmpada mesa","cor":"azul"}`
    /// e o parse morria em todas as frases. Pior, em "apaga a lâmpada" ele inventava
    /// `cor: "branco"` — inferir a intenção pelos campos presentes teria acendido a luz
    /// de branco no lugar de apagá-la.
    ///
    /// Três verbos resolvem porque o verbo é a única coisa que o schema OBRIGA. É o mesmo
    /// desenho de `volume_up`/`volume_down`/`volume_set`, que existem separados pela
    /// mesma razão e não por acaso.
    SmartHome {
        aparelho: String,
        ligar: bool,
    },
    /// "deixa a lâmpada da mesa azul". `cor` aceita nome de cor e também "quente" e
    /// "frio", que num branco são a temperatura em vez do matiz.
    SmartColor {
        aparelho: String,
        cor: String,
    },
    /// "põe a luz em 30 por cento". `nivel` de 0 a 100.
    SmartBright {
        aparelho: String,
        nivel: u8,
    },
    /// Nada a executar: conversa fiada OU pedido que não bate com nenhuma capacidade.
    /// Quem responde isso é `converse`, com histórico e memória — não este prompt.
    Reply {},
}

fn um_passo() -> u8 {
    1
}

/// Modelo pequeno esquece campo. Sem este default, `{"action":"look"}` — que é o que
/// ele emite metade das vezes para "o que é isso?" — falharia o parse inteiro e viraria
/// "não entendi", quando o certo é olhar onde fizer sentido.
fn onde_der() -> vision::Fonte {
    vision::Fonte::Auto
}

/// Fonte única da lista de verbos: alimenta o schema, e o teste quebra se algum dia
/// ela divergir do enum.
const ACOES: [&str; 28] = [
    "sou_eu",
    "smart_home",
    "smart_color",
    "smart_bright",
    "play_music",
    "weather",
    "weather_at",
    "webcam_on",
    "webcam_off",
    "look",
    "camera_on",
    "camera_off",
    "look_camera",
    "camera_move",
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
            "local":    { "type": "string" },
            "fact":     { "type": "string" },
            "about":    { "type": "string" },
            "nickname": { "type": "string" },
            "target":   { "type": "string" },
            "aparelho": { "type": "string" },
            "cor":      { "type": "string" },
            "nivel":    { "type": "integer" },
            "ligar":    { "type": "boolean" },
            "fonte":    { "type": "string", "enum": ["tela", "webcam", "auto"] },
            "camera":   { "type": "string" },
            "pessoa":   { "type": "string" },
            // Enum FECHADO, e é o que permite `camera_move` ser um verbo só em vez de
            // quatro: a gramática derivada do schema não deixa o modelo emitir outra
            // coisa, então não há campo livre para ele errar.
            "direcao":  { "type": "string", "enum": ["esquerda", "direita", "cima", "baixo"] },
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

/// O envelope do `/api/chat`. Vale para a resposta inteira e para cada pedaço do
/// fluxo — o Ollama usa a MESMA forma nos dois, e é isso que deixa [`pedir`] e
/// [`pedir_em_fluxo`] compartilharem o parse.
#[derive(Deserialize)]
struct Envelope {
    message: Mensagem,
}

#[derive(Deserialize)]
struct Mensagem {
    content: String,
}

/// O POST e a tradução dos erros, sem ler o corpo. Existe porque as duas formas de
/// pedir — de uma vez e em fluxo — diferem só na LEITURA, e um 404 tem que continuar
/// virando "esse modelo não está baixado" nas duas.
async fn postar(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    corpo: &serde_json::Value,
) -> Result<reqwest::Response, AgentError> {
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

    Ok(resposta)
}

/// POST cru ao `/api/chat`, devolvendo o `message.content`. Compartilhado com
/// `converse`, para os erros do Ollama serem traduzidos num lugar só.
pub(crate) async fn pedir(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    corpo: &serde_json::Value,
) -> Result<String, AgentError> {
    let resposta = postar(http, url, model, corpo).await?;

    let envelope: Envelope = resposta
        .json()
        .await
        .map_err(|error| rede(error, url, model))?;

    Ok(envelope.message.content)
}

/// O mesmo pedido, lido **enquanto o modelo escreve**: cada pedaço de texto passa por
/// `ao_pedaco` assim que chega da rede, e o texto inteiro volta no fim.
///
/// É o que permite falar a primeira frase antes do último token. Com `"stream": true` o
/// Ollama responde **NDJSON** — um objeto por linha, no mesmo formato do [`Envelope`],
/// com a última trazendo `done: true` e conteúdo vazio.
///
/// A montagem é por BYTE e não por texto: um pedaço da rede pode cortar um caractere
/// acentuado no meio, e "não" não pode virar "n?o" na fala. Só a linha inteira, já
/// terminada em `\n`, é decodificada — aí o UTF-8 está sempre completo.
///
/// Linha que não casa com o envelope é ignorada de propósito: o corpo de erro do Ollama
/// só aparece com HTTP de erro, que o [`postar`] já pegou antes de chegar aqui.
pub(crate) async fn pedir_em_fluxo(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    corpo: &serde_json::Value,
    mut ao_pedaco: impl FnMut(&str),
) -> Result<String, AgentError> {
    let mut resposta = postar(http, url, model, corpo).await?;

    let mut inteiro = String::new();
    let mut sobra: Vec<u8> = Vec::new();

    while let Some(bytes) = resposta
        .chunk()
        .await
        .map_err(|error| rede(error, url, model))?
    {
        sobra.extend_from_slice(&bytes);

        for texto in colher(&mut sobra) {
            inteiro.push_str(&texto);
            ao_pedaco(&texto);
        }
    }

    // A última linha pode vir sem o `\n` do fim. O `colher` só corta em quebra, então sem
    // isto o fecho da resposta se perderia — e com ele a última frase da fala.
    if let Some(texto) = conteudo(&sobra) {
        inteiro.push_str(&texto);
        ao_pedaco(&texto);
    }

    Ok(inteiro)
}

/// Tira do buffer as linhas COMPLETAS e devolve o texto de cada uma, na ordem.
///
/// O que sobra fica no buffer para o pedaço seguinte da rede juntar — é aí que mora a
/// linha cortada no meio, que é o caso normal e não a exceção.
fn colher(sobra: &mut Vec<u8>) -> Vec<String> {
    let mut pedacos = Vec::new();

    while let Some(quebra) = sobra.iter().position(|byte| *byte == b'\n') {
        let linha: Vec<u8> = sobra.drain(..=quebra).collect();
        if let Some(texto) = conteudo(&linha) {
            pedacos.push(texto);
        }
    }

    pedacos
}

/// O texto de uma linha do NDJSON, ou `None` se ela não traz conteúdo.
///
/// São dois os casos de `None`, e os dois são normais: a linha final do fluxo, que vem com
/// `done: true` e conteúdo vazio, e qualquer linha que não case com o [`Envelope`].
fn conteudo(linha: &[u8]) -> Option<String> {
    let envelope: Envelope = serde_json::from_slice(linha).ok()?;
    (!envelope.message.content.is_empty()).then_some(envelope.message.content)
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
    let acao: Intent = serde_json::from_str(texto.trim())
        .map_err(|erro| AgentError::NaoEntendi(format!("{erro} — {texto}")))?;

    Ok(sem_confundir_pergunta(acao, frase))
}

/// Desfaz o único erro do roteador que o prompt não consegue corrigir.
///
/// **Medido contra o 3B**: "quem é o Guilherme?" sai como `sou_eu` mesmo com a regra
/// escrita e um exemplo idêntico entre os poucos exemplos do prompt. Ele vê um nome
/// próprio ao lado do verbo "ser" e casa com o padrão, sem pesar o "quem" nem o "?".
///
/// Insistir no prompt sairia caro: cada frase acrescentada ali desloca o balanço de
/// comando contra conversa que o [`system_prompt`] documenta. E o custo do erro é alto —
/// `sou_eu` GRAVA o rosto de quem está na webcam sob aquele nome, então uma pergunta
/// sobre outra pessoa registraria você com o nome errado, calado.
///
/// Vale só para o `sou_eu`: é o único verbo cuja frase característica ("sou o X") é
/// gramaticalmente idêntica à de uma pergunta sobre um terceiro. Os outros não têm esse
/// gêmeo, e uma regra geral de "pergunta vira reply" quebraria o `look_camera` ("tem
/// alguém na garagem?"), que é uma pergunta de verdade.
fn sem_confundir_pergunta(acao: Intent, frase: &str) -> Intent {
    match &acao {
        Intent::SouEu { .. } if e_pergunta(frase) => Intent::Reply {},
        _ => acao,
    }
}

/// Se a frase pergunta em vez de afirmar.
///
/// O ponto de interrogação sozinho não basta: quem fala com o assistente por voz não
/// pontua, e o Whisper nem sempre põe. Por isso o pronome interrogativo no COMEÇO conta
/// igual — e só no começo, porque "sou o Bruno, quem é você?" é uma apresentação.
fn e_pergunta(frase: &str) -> bool {
    let limpa = crate::core::memory::normalizar(frase);
    let limpa = limpa.trim();

    if frase.trim_end().ends_with('?') {
        return true;
    }

    ["quem ", "qual ", "quais "]
        .iter()
        .any(|pronome| limpa.starts_with(pronome))
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
///
/// **O BALANÇO entre os três blocos é o que decide se ele executa sem ser pedido**, e
/// ele degrada sozinho: cada feature nova (música, webcam, visão) chega com exemplos de
/// COMANDO e nenhum de CONVERSA, e a razão sobe sem ninguém decidir isso. A 6:1 ele
/// pausou a música de um usuário no meio de um desabafo. Ao mexer aqui, **conte os dois
/// lados antes de reescrever regra nenhuma** — hoje são 25 comandos, 4 perguntas sobre
/// o mundo e 17 conversas (~1,5:1), e as conversas incluem de propósito frases que
/// CITAM tela e objeto sem pedir para olhar, que são os falsos amigos do `look` — e
/// agora também frases que CITAM uma luz sem mandar mexer nela, que são os do
/// `smart_home`. Reclamar de lâmpada queimada é o caso mais provável de todos.
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
weather           tempo, chuva ou temperatura ONDE ELE ESTÁ. Sem argumento nenhum.
weather_at        tempo, chuva ou temperatura numa CIDADE que ele nomeou na frase.
                  `local` = só o nome da cidade, copiado da frase dele.
webcam_on         ligar a câmera DO COMPUTADOR (a webcam, a que aponta para ele).
webcam_off        desligar a câmera do computador.
camera_on         mostrar uma câmera de SEGURANÇA da casa, que tem NOME DE LUGAR
                  (garagem, portão, quintal, sala, frente, fundos). `camera` = o nome do
                  lugar como ele falou. Se ele disser só \"a câmera\" sem lugar nenhum,
                  deixe `camera` vazio.
camera_off        fechar as câmeras de segurança.
look_camera       OLHAR uma câmera de segurança e responder sobre ela — \"tem alguém
                  na garagem?\", \"o carro está na frente?\". `camera` = o nome do lugar.
camera_move       VIRAR uma câmera de segurança. `direcao` = esquerda, direita, cima ou
                  baixo. `camera` = o nome do lugar, se ele disser.
look              OLHAR uma imagem e responder sobre ela. `fonte` = onde olhar:
                  \"webcam\" quando ele aponta algo para a câmera ou fala do que está
                  segurando; \"tela\" quando ele fala do que está NA TELA, numa janela,
                  num site ou numa mensagem de erro; \"auto\" quando ele não disser.
web_search        pesquisar sobre o MUNDO. `query` = só os termos, sem \"pesquise\" nem \"no google\".
                  Tempo, chuva e temperatura NÃO são web_search — são weather, mesmo com
                  cidade no meio da frase.
sou_eu            ele está DIZENDO QUEM ELE É — \"eu sou o Guilherme\", \"sou a Ana\",
                  \"meu nome é Bruno\". `pessoa` = só o primeiro nome, sem \"eu sou\".
                  Serve para eu guardar o rosto dele. NÃO use quando ele fala de
                  OUTRA pessoa (\"o Bruno chegou\") — isso é reply.
remember          ele MANDOU guardar algo. `fact` = o que guardar, em terceira pessoa.
forget            ele mandou esquecer algo. `about` = o assunto a apagar.
smart_home        LIGAR ou DESLIGAR um aparelho da casa (luz, lâmpada, tomada).
                  `aparelho` = o nome dele como ele falou, sem \"a\", \"o\" nem \"da\".
                  `ligar` = true para acender/ligar, false para apagar/desligar.
smart_color       trocar a COR de uma lâmpada. `cor` = o nome da cor que ele disse.
                  Também vale para \"mais quente\" e \"mais frio\", que são tons de branco.
smart_bright      trocar o BRILHO de uma lâmpada. `nivel` = 0 a 100.
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

\"câmera\" também se divide em duas, e são aparelhos diferentes. **Olhe o VERBO primeiro:**
- \"LIGAR\" ou \"DESLIGAR\" a câmera -> é sempre a WEBCAM do computador: webcam_on,
  webcam_off. Ninguém liga uma câmera de segurança, ela já está ligada.
- \"MOSTRAR\", \"ABRIR\", \"VER\" + nome de LUGAR (garagem, portão, quintal, sala, frente,
  fundos, rua) ou a palavra no PLURAL (\"as câmeras\") -> é câmera de SEGURANÇA:
  camera_on, look_camera, camera_move.
- Falar do que ELE está segurando ou mostrando -> WEBCAM: look.

sou_eu é só quando ele AFIRMA quem ELE é: a frase começa com \"eu sou\", \"sou\" ou
\"meu nome é\". PERGUNTA com \"quem\" nunca é sou_eu — \"quem é o Guilherme?\" é reply,
e falar de outra pessoa (\"o Bruno chegou\") também.

\"abre o X\" também se divide em duas, e errar aqui não abre nada:
- X é um SITE ou serviço da web (youtube, gmail, netflix, globo, chatgpt, instagram)
  -> open_site, com a URL completa.
- X é um PROGRAMA instalado no PC (spotify, notepad, calculadora, steam, discord)
  -> open_app, só o nome.

Nunca invente uma ação, e nunca invente termos que o usuário não disse.

VOCÊ NÃO FAZ TUDO. Não existe ação para curtir, favoritar ou salvar música, mexer em
playlist, ver o que está tocando, mandar mensagem, nem mexer em arquivo. Pedido desses
é reply — quem responde explica que não sabe fazer. Escolher a
ação PARECIDA é o pior erro possível: \"salva essa música\" não é play_music, e
\"qual está tocando agora\" não é media_play_pause.

Frase que fala de música TOCANDO só vira media_play_pause se ela mandar PARAR ou
RETOMAR. Citar o que está tocando é conversa.

Exemplos de COMANDO:
\"abre o youtube\"                    -> {{\"action\":\"open_site\",\"url\":\"https://www.youtube.com\"}}
\"põe o spotify pra rodar\"           -> {{\"action\":\"open_app\",\"name\":\"spotify\"}}
\"abaixa dois\"                       -> {{\"action\":\"volume_down\",\"steps\":2}}
\"deixa em 30\"                       -> {{\"action\":\"volume_set\",\"level\":30}}
\"pausa\"                             -> {{\"action\":\"media_play_pause\"}}
\"pula essa música\"                  -> {{\"action\":\"media_next\"}}
\"volta pra anterior\"                -> {{\"action\":\"media_previous\"}}
\"toque Charlie Brown Jr só os loucos sabem no spotify\" -> {{\"action\":\"play_music\",\"query\":\"Charlie Brown Jr Só os Loucos Sabem\"}}
\"coloca uma música do Djavan\"       -> {{\"action\":\"play_music\",\"query\":\"Djavan\"}}
\"liga a câmera\"                     -> {{\"action\":\"webcam_on\"}}
\"desliga a câmera\"                  -> {{\"action\":\"webcam_off\"}}
\"mostra a garagem\"                  -> {{\"action\":\"camera_on\",\"camera\":\"garagem\"}}
\"abre a câmera do portão\"           -> {{\"action\":\"camera_on\",\"camera\":\"portão\"}}
\"me mostra as câmeras\"              -> {{\"action\":\"camera_on\",\"camera\":\"\"}}
\"liga a câmera\"                     -> {{\"action\":\"webcam_on\"}}
\"quem é o Guilherme?\"               -> {{\"action\":\"reply\"}}
\"fecha as câmeras\"                  -> {{\"action\":\"camera_off\"}}
\"tem alguém na garagem?\"            -> {{\"action\":\"look_camera\",\"camera\":\"garagem\"}}
\"o carro tá na frente?\"             -> {{\"action\":\"look_camera\",\"camera\":\"frente\"}}
\"vira a câmera pra esquerda\"        -> {{\"action\":\"camera_move\",\"camera\":\"\",\"direcao\":\"esquerda\"}}
\"olha mais pra cima no quintal\"     -> {{\"action\":\"camera_move\",\"camera\":\"quintal\",\"direcao\":\"cima\"}}
\"o que é isso?\"                     -> {{\"action\":\"look\",\"fonte\":\"auto\"}}
\"que objeto é esse na minha mão\"    -> {{\"action\":\"look\",\"fonte\":\"webcam\"}}
\"que mouse é esse?\"                 -> {{\"action\":\"look\",\"fonte\":\"webcam\"}}
\"o que tem na minha tela?\"          -> {{\"action\":\"look\",\"fonte\":\"tela\"}}
\"que erro é esse aí na tela\"        -> {{\"action\":\"look\",\"fonte\":\"tela\"}}
\"lê isso pra mim\"                   -> {{\"action\":\"look\",\"fonte\":\"tela\"}}
\"eu sou o Guilherme\"                -> {{\"action\":\"sou_eu\",\"pessoa\":\"Guilherme\"}}
\"meu nome é Ana\"                    -> {{\"action\":\"sou_eu\",\"pessoa\":\"Ana\"}}
\"lembra que eu acordo 6h30\"         -> {{\"action\":\"remember\",\"fact\":\"Acorda 6h30.\"}}
\"esquece a academia\"                -> {{\"action\":\"forget\",\"about\":\"academia\"}}
\"meu jogo é o steam\"                -> {{\"action\":\"alias\",\"nickname\":\"meu jogo\",\"target\":\"steam\"}}
\"apaga a luz da cozinha\"            -> {{\"action\":\"smart_home\",\"aparelho\":\"luz cozinha\",\"ligar\":false}}
\"acende a lâmpada do quarto\"        -> {{\"action\":\"smart_home\",\"aparelho\":\"lâmpada quarto\",\"ligar\":true}}
\"muda a lâmpada mesa para azul\"     -> {{\"action\":\"smart_color\",\"aparelho\":\"lâmpada mesa\",\"cor\":\"azul\"}}
\"deixa a luz da sala vermelha\"      -> {{\"action\":\"smart_color\",\"aparelho\":\"luz sala\",\"cor\":\"vermelho\"}}
\"põe a lâmpada mesa em 30 por cento\" -> {{\"action\":\"smart_bright\",\"aparelho\":\"lâmpada mesa\",\"nivel\":30}}

Exemplos de PERGUNTA SOBRE O MUNDO — vão para web_search:
\"pesquisa no google quem foi tesla\" -> {{\"action\":\"web_search\",\"query\":\"nikola tesla\"}}
\"quem descobriu o brasil?\"          -> {{\"action\":\"web_search\",\"query\":\"descobrimento do brasil\"}}
\"o que é rust?\"                     -> {{\"action\":\"web_search\",\"query\":\"rust linguagem de programação\"}}
\"como faz pão de queijo\"            -> {{\"action\":\"web_search\",\"query\":\"receita de pão de queijo\"}}

Exemplos de CONVERSA — todos reply, mesmo citando música, jogo, tela ou objeto:
\"po enquanto nada, quero é ir pra casa pra poder jogar\" -> {{\"action\":\"reply\"}}
\"não pedi nada pra vc, to apenas conversando\"           -> {{\"action\":\"reply\"}}
\"essa música que tocou agora é boa demais\"              -> {{\"action\":\"reply\"}}
\"o volume do meu fone tá estourando os ouvidos\"         -> {{\"action\":\"reply\"}}
\"passei o dia todo no vscode\"                           -> {{\"action\":\"reply\"}}
\"acho o youtube viciante demais\"                        -> {{\"action\":\"reply\"}}
\"salve essa musica nas minhas curtidas, gostei dela\"     -> {{\"action\":\"reply\"}}
\"e pra add a que esta tocando agora, não a que vc salvou\" -> {{\"action\":\"reply\"}}
\"que musica é essa que ta tocando?\"                      -> {{\"action\":\"reply\"}}
\"tela azul de novo, que ódio\"                           -> {{\"action\":\"reply\"}}
\"esse mouse aqui já era, tá com o clique falhando\"      -> {{\"action\":\"reply\"}}
\"não tô vendo a hora de acabar esse projeto\"            -> {{\"action\":\"reply\"}}
\"minha tela tá pequena demais pra trabalhar\"            -> {{\"action\":\"reply\"}}
\"a luz da cozinha tá queimada de novo\"                  -> {{\"action\":\"reply\"}}
\"esqueci a luz da sala acesa a noite toda\"              -> {{\"action\":\"reply\"}}
\"azul é a minha cor favorita\"                           -> {{\"action\":\"reply\"}}
\"a luz dessa sala é muito fraca pra ler\"                -> {{\"action\":\"reply\"}}
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
            (r#"{"action":"weather"}"#, Intent::Weather {}),
            (
                r#"{"action":"weather_at","local":"Lisboa"}"#,
                Intent::WeatherAt {
                    local: "Lisboa".to_owned(),
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
            (
                r#"{"action":"smart_home","aparelho":"luz cozinha","ligar":false}"#,
                Intent::SmartHome {
                    aparelho: "luz cozinha".to_owned(),
                    ligar: false,
                },
            ),
            (
                r#"{"action":"smart_color","aparelho":"lâmpada mesa","cor":"azul"}"#,
                Intent::SmartColor {
                    aparelho: "lâmpada mesa".to_owned(),
                    cor: "azul".to_owned(),
                },
            ),
            (
                r#"{"action":"smart_bright","aparelho":"luz sala","nivel":30}"#,
                Intent::SmartBright {
                    aparelho: "luz sala".to_owned(),
                    nivel: 30,
                },
            ),
            (r#"{"action":"webcam_on"}"#, Intent::WebcamOn {}),
            (r#"{"action":"webcam_off"}"#, Intent::WebcamOff {}),
            // Sem `fonte`: é o que o 3B emite metade das vezes, e o default tem que
            // segurar isso — a alternativa é "não entendi" para "o que é isso?".
            (
                r#"{"action":"look"}"#,
                Intent::Look {
                    fonte: vision::Fonte::Auto,
                },
            ),
            // Sem `camera`: é o que sai de "me mostra as câmeras", e o default vazio tem
            // que segurar — quem resolve "vazio com uma câmera só" é o catálogo.
            (
                r#"{"action":"camera_on"}"#,
                Intent::CameraOn {
                    camera: String::new(),
                },
            ),
            (r#"{"action":"camera_off"}"#, Intent::CameraOff {}),
            (
                r#"{"action":"look_camera","camera":"garagem"}"#,
                Intent::LookCamera {
                    camera: "garagem".to_owned(),
                },
            ),
            (
                r#"{"action":"camera_move","camera":"quintal","direcao":"esquerda"}"#,
                Intent::CameraMove {
                    camera: "quintal".to_owned(),
                    direcao: cameras::onvif::Direcao::Esquerda,
                },
            ),
            (
                r#"{"action":"sou_eu","pessoa":"Guilherme"}"#,
                Intent::SouEu {
                    pessoa: "Guilherme".to_owned(),
                },
            ),
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

        let propriedades = schema["properties"]
            .as_object()
            .expect("o schema precisa listar as propriedades");

        for (json, esperado) in amostras {
            let intent: Intent = serde_json::from_str(json)
                .unwrap_or_else(|error| panic!("não parseou {json}: {error}"));
            assert_eq!(intent, esperado);

            // **Todo campo da amostra tem que existir no schema.** O Ollama usa este
            // schema para RESTRINGIR a geração: um campo que não está aqui é impossível
            // de emitir, por mais claro que o prompt seja. Aconteceu com o `local` do
            // `weather` — o modelo roteava certo e devolvia a cidade vazia sempre, e a
            // culpa parecia ser dele.
            let amostra: serde_json::Value = serde_json::from_str(json).expect("json");
            for campo in amostra.as_object().expect("objeto").keys() {
                assert!(
                    propriedades.contains_key(campo),
                    "o campo {campo} não está no schema, então o modelo não consegue emiti-lo"
                );
            }

            let verbo = serde_json::to_value(&intent).expect("serializa")["action"].clone();
            assert!(acoes.contains(&verbo), "{verbo} não está no schema");
        }

        // O portão de verdade é o serde, não o schema: campo obrigatório faltando
        // TEM que falhar, para virar `NaoEntendi` em vez de uma ação sem alvo.
        assert!(serde_json::from_str::<Intent>(r#"{"action":"open_site"}"#).is_err());
        assert!(serde_json::from_str::<Intent>(r#"{"action":"alias","nickname":"x"}"#).is_err());
        assert!(serde_json::from_str::<Intent>(r#"{"action":"voar"}"#).is_err());
    }

    /// A `fonte` é a única coisa que só o roteador consegue decidir, então ela precisa
    /// atravessar o parse inteira — e uma fonte inventada tem que falhar, senão "olha
    /// na minha impressora" viraria um `look` sem imagem nenhuma.
    #[test]
    fn a_fonte_do_look_atravessa_o_parse() {
        let tela: Intent =
            serde_json::from_str(r#"{"action":"look","fonte":"tela"}"#).expect("tela");
        assert_eq!(
            tela,
            Intent::Look {
                fonte: vision::Fonte::Tela
            }
        );

        let webcam: Intent =
            serde_json::from_str(r#"{"action":"look","fonte":"webcam"}"#).expect("webcam");
        assert_eq!(
            webcam,
            Intent::Look {
                fonte: vision::Fonte::Webcam
            }
        );

        assert!(
            serde_json::from_str::<Intent>(r#"{"action":"look","fonte":"impressora"}"#).is_err()
        );
    }

    /// O balanço de exemplos é o que impede comando falso, e degrada sozinho a cada
    /// feature nova. Este teste não julga o número certo — ele só quebra quando alguém
    /// empilha exemplos de COMANDO sem contrapeso, que foi exatamente como a razão
    /// subiu de 10:9 para 22:9 sem ninguém decidir isso.
    #[test]
    fn os_exemplos_de_comando_nao_afogam_os_de_conversa() {
        let prompt = system_prompt("Jarvis", &BTreeMap::new());

        let comandos = bloco(&prompt, "Exemplos de COMANDO:");
        let conversas = bloco(&prompt, "Exemplos de CONVERSA");

        assert!(comandos > 0 && conversas > 0, "os blocos sumiram do prompt");
        assert!(
            comandos <= conversas * 2,
            "{comandos} exemplos de comando contra {conversas} de conversa: \
             acrescente conversas antes de acrescentar comandos"
        );
    }

    /// Conta as linhas de exemplo (`"frase" -> {…}`) de um bloco até a linha em branco.
    fn bloco(prompt: &str, titulo: &str) -> usize {
        prompt
            .split_once(titulo)
            .map(|(_, resto)| resto)
            .unwrap_or_default()
            .lines()
            .skip(1)
            .take_while(|linha| !linha.trim().is_empty())
            .filter(|linha| linha.contains("->"))
            .count()
    }

    /// Pergunta ao modelo de verdade e imprime o que ele decidiu.
    ///
    /// Fora do `cargo test` comum porque depende do Ollama de pé. É a única forma de
    /// saber se um verbo novo é APRENDÍVEL — o teste de mesa prova que o schema e o enum
    /// concordam, e não que um modelo de 3B consegue preencher os campos.
    ///
    /// `cargo test --lib -- --ignored --nocapture interpreta_de_verdade`
    #[test]
    #[ignore]
    fn interpreta_de_verdade() {
        let frases = [
            "mude a Lâmpada Mesa para a cor azul",
            "deixa a lâmpada mesa vermelha",
            "apaga a lâmpada mesa",
            "acende a lâmpada mesa",
            "põe a lâmpada mesa em 20 por cento",
            "deixa a luz mais quente",
            "a luz dessa sala é muito fraca pra ler",
            "azul é a minha cor favorita",
            // O tempo, nas formas em que a pergunta realmente aparece. As três primeiras
            // têm que sair com `local` VAZIO, e as duas últimas com a cidade — confundir
            // as duas coisas é o jeito mais fácil deste verbo nascer quebrado.
            "como está o tempo?",
            "vai chover hoje?",
            "que temperatura tá fazendo aí fora",
            "como está o tempo em Lisboa",
            "vai chover amanhã em São Paulo?",
        ];

        let http = client();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                for frase in frases {
                    let saida = interpret(
                        &http,
                        "http://localhost:11434",
                        "qwen2.5vl:3b",
                        "Jarvis",
                        &BTreeMap::new(),
                        frase,
                    )
                    .await;

                    match saida {
                        Ok(acao) => println!("{frase:<42} -> {acao:?}"),
                        Err(erro) => println!("{frase:<42} -> ERRO {erro}"),
                    }
                }
            });
    }

    /// O mesmo, para os verbos de câmera — e principalmente para o que eles podem
    /// QUEBRAR.
    ///
    /// Duas coisas se medem aqui, e a segunda é a que mais importa:
    ///
    /// 1. Os verbos novos são aprendíveis — "mostra a garagem" vira `camera_on`, e não
    ///    `webcam_on` nem `open_app`.
    /// 2. **O campo `camera` não vaza.** É o risco que este módulo documenta em dois
    ///    lugares: um campo declarado é um campo que a gramática deixa emitir, e o modelo
    ///    prefere emitir a omitir. As frases do fim não falam de câmera nenhuma, e
    ///    qualquer `camera` preenchido nelas é o sintoma de que a lição foi violada.
    ///
    /// `cargo test --lib -- --ignored --nocapture as_cameras_sao_aprendiveis`
    #[test]
    #[ignore]
    fn as_cameras_sao_aprendiveis() {
        let frases = [
            // Devem virar os verbos de câmera de segurança.
            "mostra a garagem",
            "abre a câmera do portão",
            "me mostra as câmeras",
            "fecha as câmeras",
            "tem alguém na garagem?",
            "o carro tá na frente?",
            "vira a câmera pra esquerda",
            "olha mais pra cima no quintal",
            // A fronteira com a WEBCAM: sem lugar nenhum, é a câmera do computador.
            "liga a câmera",
            "desliga a câmera",
            "que mouse é esse?",
            // Apresentar-se: vira `sou_eu`, para o rosto ser guardado.
            "eu sou o Guilherme",
            "meu nome é Ana",
            "sou o Bruno",
            // A fronteira do `sou_eu`: falar de OUTRA pessoa não é se apresentar, e
            // guardar o rosto de quem está na câmera com o nome de um terceiro é o erro
            // mais confuso que essa feature pode cometer.
            "o Bruno chegou",
            "quem é o Guilherme?",
            // As armadilhas: nada disso é câmera, e nenhuma pode sair com `camera`
            // preenchido nem virar um verbo de câmera.
            "que horas são?",
            "abaixa o volume",
            "como está o tempo?",
            "abre o youtube",
            "apaga a luz da cozinha",
            "tô cansado hoje",
        ];

        let http = client();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                for frase in frases {
                    let saida = interpret(
                        &http,
                        "http://localhost:11434",
                        "qwen2.5vl:3b",
                        "Jarvis",
                        &BTreeMap::new(),
                        frase,
                    )
                    .await;

                    match saida {
                        Ok(acao) => println!("{frase:<34} -> {acao:?}"),
                        Err(erro) => println!("{frase:<34} -> ERRO {erro}"),
                    }
                }
            });
    }

    /// O guarda contra o único erro que o prompt não corrigiu — e o custo dele é gravar
    /// o rosto de quem está na webcam sob o nome de outra pessoa, calado.
    #[test]
    fn pergunta_sobre_alguem_nao_vira_apresentacao() {
        let sou_eu = || Intent::SouEu {
            pessoa: "Guilherme".to_owned(),
        };

        // O caso medido: o 3B devolve `sou_eu` para isto mesmo com exemplo no prompt.
        assert_eq!(
            sem_confundir_pergunta(sou_eu(), "quem é o Guilherme?"),
            Intent::Reply {}
        );
        // Sem pontuação, que é como a voz chega depois do Whisper.
        assert_eq!(
            sem_confundir_pergunta(sou_eu(), "quem e o Guilherme"),
            Intent::Reply {}
        );

        // A apresentação de verdade passa intacta.
        assert_eq!(
            sem_confundir_pergunta(sou_eu(), "eu sou o Guilherme"),
            sou_eu()
        );
        assert_eq!(
            sem_confundir_pergunta(sou_eu(), "meu nome é Guilherme"),
            sou_eu()
        );
    }

    /// Uma pergunta que segue a apresentação não pode anular o cadastro: o "quem" só
    /// conta no COMEÇO da frase.
    #[test]
    fn pronome_no_meio_da_frase_nao_conta() {
        let acao = Intent::SouEu {
            pessoa: "Bruno".to_owned(),
        };

        assert_eq!(
            sem_confundir_pergunta(acao.clone(), "sou o Bruno, e você quem é"),
            acao
        );
    }

    /// O guarda vale SÓ para o `sou_eu`. Uma regra geral de "pergunta vira reply"
    /// quebraria o `look_camera`, cuja frase típica é justamente uma pergunta.
    #[test]
    fn o_guarda_nao_encosta_nos_outros_verbos() {
        let olhar = Intent::LookCamera {
            camera: "garagem".to_owned(),
        };
        assert_eq!(
            sem_confundir_pergunta(olhar.clone(), "tem alguém na garagem?"),
            olhar
        );

        let tempo = Intent::Weather {};
        assert_eq!(
            sem_confundir_pergunta(tempo.clone(), "qual o tempo hoje?"),
            tempo
        );
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

    /// Uma resposta em fluxo do Ollama, como ela sai do `/api/chat` com `"stream": true`.
    const FLUXO: &str = r#"{"message":{"role":"assistant","content":"Bom dia"},"done":false}
{"message":{"role":"assistant","content":", Guilherme"},"done":false}
{"message":{"role":"assistant","content":". Está tudo em ordem."},"done":false}
{"message":{"role":"assistant","content":""},"done":true,"total_duration":1}
"#;

    /// **A rede não respeita linha.** Um pedaço pode acabar no meio do JSON, no meio de uma
    /// palavra, e — o caso que estraga tudo — no meio de um caractere acentuado: os dois
    /// bytes de "ó" chegam separados. Só a linha inteira é decodificada, e é isso que
    /// impede "Está" de virar "Est?".
    ///
    /// O teste corta o fluxo byte a byte, que é o pior corte possível, e cobra o mesmo
    /// texto que sairia se ele tivesse chegado de uma vez.
    #[test]
    fn o_fluxo_remonta_as_linhas_cortadas_pela_rede() {
        let mut sobra: Vec<u8> = Vec::new();
        let mut pedacos: Vec<String> = Vec::new();

        for byte in FLUXO.as_bytes() {
            sobra.push(*byte);
            pedacos.extend(colher(&mut sobra));
        }

        assert_eq!(pedacos, ["Bom dia", ", Guilherme", ". Está tudo em ordem."]);
        assert!(sobra.is_empty(), "o buffer não pode segurar linha completa");
    }

    /// A linha final do fluxo vem com `done: true` e conteúdo vazio — não é pedaço de fala,
    /// e emiti-la faria uma frase vazia ir parar na fila do motor de voz.
    #[test]
    fn a_linha_de_fecho_nao_vira_fala() {
        assert_eq!(conteudo(br#"{"message":{"content":""},"done":true}"#), None);
        assert_eq!(
            conteudo(br#"{"message":{"content":"oi"},"done":false}"#).as_deref(),
            Some("oi")
        );

        // Linha que não é envelope nenhum não pode derrubar o turno: some, e o resto do
        // fluxo continua.
        assert_eq!(conteudo(b"nao e json"), None);
    }
}
