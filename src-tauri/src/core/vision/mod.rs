//! Olhar — pela webcam ou para a tela — e responder o que foi perguntado.
//!
//! **Um modelo local só para tudo, e isso foi medido.** A ideia óbvia era manter o
//! `qwen2.5:3b` para texto e somar um modelo de visão pequeno. Não funciona nesta
//! máquina: com 4 GB de VRAM o Ollama não segura os dois, e a primeira chamada ao
//! `moondream` levou **67 segundos** — o tempo de descarregar um e carregar o outro.
//! Como a mensagem seguinte é texto, ela pagaria a troca de volta.
//!
//! Então o modelo do intérprete precisa ser multimodal, e a visão local usa o MESMO
//! modelo já quente.
//!
//! **E um 3B tem teto.** Ele descreve bem uma cena ("um mouse preto sobre a mesa"),
//! mas erra em identificar QUAL mouse e em ler texto pequeno numa captura de tela —
//! 810 no OCRBench contra 883 do 7B, com erros de ordem de leitura em layout denso. E
//! ele erra sem avisar, que é o modo de falha caro. Por isso, quando existe uma chave
//! da Anthropic nas configurações, a imagem vai para lá; sem chave, nada muda em
//! relação ao que o app já fazia.

mod claude;
mod ollama;

use serde::{Deserialize, Serialize};

use super::agent::AgentError;
use crate::config::AppSettings;

/// De onde veio a imagem. Muda o prompt (o que o modelo deve esperar ver) e, no
/// `Auto`, é resolvido pelo estado da webcam em vez de por adivinhação do roteador.
///
/// `Serialize` porque o log do chat renderiza os argumentos da ação a partir do próprio
/// `Intent` — sem isso, a linha `AÇÃO look` não diria para onde ele olhou, que é a
/// primeira coisa que se quer saber quando ele responde sobre a coisa errada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fonte {
    Tela,
    Webcam,
    /// "o que é isso?" não diz onde olhar. Quem decide é [`Fonte::resolver`].
    Auto,
}

impl Fonte {
    /// Webcam ligada é uma declaração de intenção: ninguém abre a câmera e pergunta
    /// sobre a tela. Com ela fechada, a tela é o único lugar onde há o que ver.
    pub fn resolver(self, webcam_aberta: bool) -> Self {
        match self {
            Self::Auto if webcam_aberta => Self::Webcam,
            Self::Auto => Self::Tela,
            outra => outra,
        }
    }

    /// Como a fonte é descrita PARA O MODELO. É o que faz "o que eu tô segurando"
    /// não ser respondido com o conteúdo de uma janela.
    fn descricao(self) -> &'static str {
        match self {
            Self::Webcam | Self::Auto => "a imagem da webcam",
            Self::Tela => "esta captura da tela do computador",
        }
    }
}

/// O que a visão devolve.
///
/// `buscar` é a parte que não é óbvia: um modelo pode VER "Comic Con Experience 2026"
/// num cartaz e ainda assim não saber quando os ingressos abrem — isso não está na
/// imagem. Sem uma saída para dizer isso, ele inventa uma data. O campo é essa saída:
/// preenchido, significa "identifiquei a coisa, o resto está fora da imagem", e quem
/// chama sabe que precisa pesquisar.
///
/// Vazio = não precisa buscar, mesmo padrão de `ollamaModel` e `ttsVoiceId` nas
/// configurações (vazio significa "não se aplica", em vez de um booleano ao lado).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Visao {
    pub resposta: String,
    #[serde(default)]
    pub buscar: String,
}

/// O `data:` URL que a captura devolve não serve para as APIs — elas querem só o base64.
///
/// Separado e testado porque o erro é silencioso: mandar o prefixo junto faz o modelo
/// receber lixo e descrever qualquer coisa, sem nunca reclamar. Serve igual para o
/// JPEG da webcam e para o PNG da tela — ele corta na primeira vírgula, não olha o mime.
pub fn so_o_base64(data_url: &str) -> &str {
    match data_url.split_once(',') {
        Some((cabecalho, dados)) if cabecalho.starts_with("data:") => dados,
        _ => data_url,
    }
}

/// Responde `pergunta` olhando `imagem_base64`.
///
/// Com chave da Anthropic, Claude; sem chave — ou se ele falhar — o modelo local. O
/// fallback não é elegância, é o requisito: rede caindo ou cota estourada não pode
/// deixar o Jarvis cego, e o modelo local sempre está lá.
///
/// Não virou `trait` (como o `TtsEngine` da voz) porque são dois caminhos e um deles já
/// é o fallback do outro — seria uma interface com duas impls e nenhuma terceira no
/// horizonte. O `if` diz a mesma coisa em menos linhas.
pub async fn ver(
    http: &reqwest::Client,
    settings: &AppSettings,
    imagem: &Imagem<'_>,
    pergunta: &str,
    fonte: Fonte,
) -> Result<Visao, AgentError> {
    if !settings.anthropic_api_key.trim().is_empty() {
        match claude::ver(http, &settings.anthropic_api_key, imagem, pergunta, fonte).await {
            Ok(visao) => return Ok(visao),
            // Não derruba a pergunta: cai no local e registra o motivo, senão "sem
            // internet" e "chave errada" viram o mesmo silêncio.
            Err(erro) => eprintln!("[jarvis] visão pela Anthropic falhou, usando o local: {erro}"),
        }
    }

    ollama::ver(
        http,
        &settings.ollama_url,
        &settings.ollama_model,
        imagem,
        pergunta,
        fonte,
    )
    .await
}

/// A imagem e o que ela é, já sem o prefixo `data:`.
pub struct Imagem<'a> {
    pub base64: &'a str,
    /// `image/jpeg` da webcam, `image/png` da tela. O Ollama ignora; a Anthropic exige.
    pub mime: &'a str,
}

impl<'a> Imagem<'a> {
    /// Extrai as duas coisas do `data:` URL que a captura devolveu.
    pub fn do_data_url(data_url: &'a str) -> Self {
        let mime = data_url
            .strip_prefix("data:")
            .and_then(|resto| resto.split(';').next())
            .filter(|mime| mime.starts_with("image/"))
            .unwrap_or("image/jpeg");

        Self {
            base64: so_o_base64(data_url),
            mime,
        }
    }
}

/// O texto que vai junto da imagem, igual para os dois backends.
///
/// Pergunta vazia cai em "descreva" — é o comportamento que o app já tinha quando
/// `Look` não carregava nada, e continua sendo o certo para "olha isso aqui".
fn prompt(pergunta: &str, fonte: Fonte) -> String {
    let pedido = match pergunta.trim() {
        "" => "Diga o que você está vendo. Comece pelo objeto ou pela pessoa principal, e só depois pelo cenário, se valer a pena.".to_owned(),
        pergunta => format!("Responda a esta pergunta: {pergunta}"),
    };

    format!(
        "Olhe {} e responda EM PORTUGUÊS, em uma ou duas frases.\n\n\
         {pedido}\n\n\
         Se estiver escuro, borrado ou sem nada reconhecível, diga isso — não invente. \
         Não descreva pixel por pixel e não comente a qualidade da imagem.\n\n\
         Preencha `buscar` APENAS se responder depender de informação que não está na \
         imagem — data, preço, onde comprar, ficha técnica, notícia. Nesse caso ponha em \
         `buscar` o termo que identifica a coisa (o nome do evento, a marca e o modelo \
         do produto) e deixe em `resposta` o que você conseguiu ver. Se a imagem já \
         responde, deixe `buscar` vazio.",
        fonte.descricao()
    )
}

/// O schema que os dois backends impõem à saída. Um objeto plano de duas strings —
/// nada de aninhamento, pelo mesmo motivo do roteador: schema complicado degrada a
/// grammar de um modelo pequeno.
fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "resposta": { "type": "string" },
            "buscar": { "type": "string" },
        },
        "required": ["resposta", "buscar"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mandar o prefixo `data:` junto não dá erro: o modelo recebe lixo e descreve
    /// qualquer coisa. É o tipo de bug que só aparece como "ele viu errado".
    #[test]
    fn tira_o_prefixo_do_data_url() {
        assert_eq!(so_o_base64("data:image/jpeg;base64,AAAA"), "AAAA");
        assert_eq!(so_o_base64("data:image/png;base64,QUJD"), "QUJD");

        // Já sem prefixo, passa direto — e uma vírgula solta no base64 não engana.
        assert_eq!(so_o_base64("AAAA"), "AAAA");
        assert_eq!(so_o_base64("AA,AA"), "AA,AA");
    }

    /// O mime errado faz a Anthropic recusar a imagem inteira, então ele sai do
    /// `data:` URL em vez de ser assumido pela origem.
    #[test]
    fn o_mime_sai_do_proprio_data_url() {
        assert_eq!(
            Imagem::do_data_url("data:image/png;base64,QQ==").mime,
            "image/png"
        );
        assert_eq!(
            Imagem::do_data_url("data:image/jpeg;base64,QQ==").mime,
            "image/jpeg"
        );

        // Sem prefixo reconhecível, o palpite é o formato da webcam — que é de onde
        // vem toda imagem que não passou pela captura de tela.
        assert_eq!(Imagem::do_data_url("QQ==").mime, "image/jpeg");
    }

    #[test]
    fn auto_segue_a_webcam_mas_uma_fonte_explicita_manda() {
        assert_eq!(Fonte::Auto.resolver(true), Fonte::Webcam);
        assert_eq!(Fonte::Auto.resolver(false), Fonte::Tela);

        // Pedir a tela com a câmera ligada continua sendo a tela.
        assert_eq!(Fonte::Tela.resolver(true), Fonte::Tela);
        assert_eq!(Fonte::Webcam.resolver(false), Fonte::Webcam);
    }

    #[test]
    fn o_prompt_diz_de_onde_veio_a_imagem() {
        let tela = prompt("que erro é esse?", Fonte::Tela);
        assert!(tela.contains("captura da tela"));
        assert!(tela.contains("que erro é esse?"));

        // Sem pergunta, volta a ser "descreva" — o comportamento antigo do `look`.
        let webcam = prompt("  ", Fonte::Webcam);
        assert!(webcam.contains("webcam"));
        assert!(webcam.contains("Diga o que você está vendo"));
    }
}
