//! Busca na web, para o Jarvis responder o que não está na memória dele.
//!
//! **Duas fontes, e a escolha foi medida.** O que eu queria era raspar um buscador e
//! não depender de chave nenhuma. Não dá:
//!
//! | fonte | resultado |
//! | --- | --- |
//! | `html.duckduckgo.com` | devolve página de desafio anti-bot (`anomaly`, `challenge-form`) |
//! | `api.duckduckgo.com` (Instant Answer) | `AbstractText` vazio em 4 de 4 consultas reais |
//! | instâncias públicas de SearXNG | JSON desligado, 403 ou "Too Many Requests" |
//! | **Wikipedia** | **3 acertos em 4** — falha só no que não é enciclopédico |
//!
//! Então: **Wikipedia por padrão**, sem configurar nada, e uma chave do Brave Search
//! nas configurações que troca a fonte por busca web de verdade. O Brave é o que
//! resolve "preço do dólar hoje" — a Wikipedia responde isso com "Opções (título)",
//! que foi o que ela devolveu no teste.
//!
//! ponytail: sem cache. Perguntar duas vezes bate duas vezes na rede. Um mapa com TTL
//! entra quando (e se) isso incomodar.

use serde::Deserialize;

/// Quantos resultados vão para o modelo resumir. Mais que isso enche o contexto de um
/// 3B sem melhorar o resumo.
const QUANTOS: usize = 3;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("não entendi o que pesquisar")]
    SemConsulta,
    #[error("não achei nada sobre \"{0}\"")]
    NadaEncontrado(String),
    #[error("sem internet para pesquisar: {0}")]
    Rede(String),
    #[error(
        "a busca recusou a chamada (HTTP {status}). Confira a chave do Brave em Configurações"
    )]
    Recusada { status: u16 },
}

/// Um resultado, já limpo o suficiente para ir ao prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    pub titulo: String,
    pub trecho: String,
    pub url: String,
}

/// A Wikipedia recusa chamada sem User-Agent identificável.
const AGENTE: &str = "Jarvis/0.1 (assistente pessoal local)";

/// Busca, preferindo o Brave quando há chave.
pub async fn pesquisar(
    http: &reqwest::Client,
    consulta: &str,
    chave_brave: &str,
) -> Result<Vec<Achado>, SearchError> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return Err(SearchError::SemConsulta);
    }

    let achados = if chave_brave.trim().is_empty() {
        wikipedia(http, consulta).await?
    } else {
        brave(http, consulta, chave_brave.trim()).await?
    };

    if achados.is_empty() {
        return Err(SearchError::NadaEncontrado(consulta.to_owned()));
    }
    Ok(achados)
}

/// Nome da fonte, para o log de ações dizer de onde veio a resposta.
pub fn fonte(chave_brave: &str) -> &'static str {
    if chave_brave.trim().is_empty() {
        "wikipedia"
    } else {
        "brave"
    }
}

// ---- Wikipedia ---------------------------------------------------------------

/// Duas etapas: a API de busca dá os títulos, e a REST dá o resumo limpo de cada um.
///
/// Os snippets da própria busca viriam com `<span class="searchmatch">` no meio, e o
/// resumo da REST já vem em texto corrido — não vale escrever um removedor de tags
/// para piorar o texto.
async fn wikipedia(http: &reqwest::Client, consulta: &str) -> Result<Vec<Achado>, SearchError> {
    #[derive(Deserialize)]
    struct Busca {
        query: Resultados,
    }
    #[derive(Deserialize)]
    struct Resultados {
        search: Vec<Titulo>,
    }
    #[derive(Deserialize)]
    struct Titulo {
        title: String,
    }

    let url = format!(
        "https://pt.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit={QUANTOS}&format=json",
        urlencode(consulta)
    );

    let busca: Busca = pegar(http, &url).await?;

    let mut achados = Vec::new();
    for titulo in busca.query.search.into_iter().take(QUANTOS) {
        if let Some(achado) = resumo_da_wikipedia(http, &titulo.title).await {
            achados.push(achado);
        }
    }

    Ok(achados)
}

/// Página sem resumo é pulada em silêncio: uma falha em uma das três não pode derrubar
/// a busca inteira.
async fn resumo_da_wikipedia(http: &reqwest::Client, titulo: &str) -> Option<Achado> {
    #[derive(Deserialize)]
    struct Resumo {
        title: String,
        #[serde(default)]
        extract: String,
        #[serde(default)]
        content_urls: Option<Enderecos>,
    }
    #[derive(Deserialize)]
    struct Enderecos {
        desktop: Desktop,
    }
    #[derive(Deserialize)]
    struct Desktop {
        page: String,
    }

    let url = format!(
        "https://pt.wikipedia.org/api/rest_v1/page/summary/{}",
        urlencode(titulo)
    );

    let resumo: Resumo = pegar(http, &url).await.ok()?;
    if resumo.extract.trim().is_empty() {
        return None;
    }

    Some(Achado {
        titulo: resumo.title,
        trecho: resumo.extract,
        url: resumo
            .content_urls
            .map(|enderecos| enderecos.desktop.page)
            .unwrap_or_default(),
    })
}

// ---- Brave -------------------------------------------------------------------

async fn brave(
    http: &reqwest::Client,
    consulta: &str,
    chave: &str,
) -> Result<Vec<Achado>, SearchError> {
    #[derive(Deserialize)]
    struct Resposta {
        #[serde(default)]
        web: Option<Web>,
    }
    #[derive(Deserialize)]
    struct Web {
        #[serde(default)]
        results: Vec<Resultado>,
    }
    #[derive(Deserialize)]
    struct Resultado {
        title: String,
        #[serde(default)]
        description: String,
        url: String,
    }

    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}&count={QUANTOS}&country=BR&search_lang=pt",
        urlencode(consulta)
    );

    let resposta = http
        .get(&url)
        // Timeout por requisição: o cliente compartilhado é o do intérprete, que tem
        // 3 minutos de folga para carregar modelo na VRAM. Busca travada por 3 minutos
        // seria o app inteiro parado.
        .timeout(TIMEOUT)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", chave)
        .send()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(SearchError::Recusada {
            status: status.as_u16(),
        });
    }

    let corpo: Resposta = resposta
        .json()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))?;

    Ok(corpo
        .web
        .map(|web| web.results)
        .unwrap_or_default()
        .into_iter()
        .map(|resultado| Achado {
            titulo: resultado.title,
            trecho: limpar(&resultado.description),
            url: resultado.url,
        })
        .collect())
}

// ---- utilidades --------------------------------------------------------------

async fn pegar<T: for<'de> Deserialize<'de>>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T, SearchError> {
    let resposta = http
        .get(url)
        .timeout(TIMEOUT)
        .header("User-Agent", AGENTE)
        .send()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(SearchError::Recusada {
            status: status.as_u16(),
        });
    }

    resposta
        .json()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))
}

/// O Brave devolve `<strong>` nos trechos para marcar o termo casado. Tirar tag é
/// mais barato que ensinar o modelo a ignorá-la.
fn limpar(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut dentro_de_tag = false;

    for c in texto.chars() {
        match c {
            '<' => dentro_de_tag = true,
            '>' => dentro_de_tag = false,
            outro if !dentro_de_tag => saida.push(outro),
            _ => {}
        }
    }

    saida.trim().to_owned()
}

/// Percent-encode dos caracteres que quebram uma query string. Não vale declarar o
/// `url` aqui: são dez linhas, e o `Url::parse_with_params` do `core::system` monta
/// URL, enquanto aqui é interpolação em template.
fn urlencode(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len() * 3);

    for byte in texto.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                saida.push(*byte as char);
            }
            _ => saida.push_str(&format!("%{byte:02X}")),
        }
    }

    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acento na consulta é o caso normal em português, e concatenar sem encode
    /// devolveria 400 da Wikipedia.
    #[test]
    fn codifica_a_consulta_inclusive_com_acento() {
        assert_eq!(urlencode("pao de queijo"), "pao%20de%20queijo");
        assert_eq!(urlencode("preço"), "pre%C3%A7o");
        assert_eq!(urlencode("c++ & rust"), "c%2B%2B%20%26%20rust");
    }

    /// Sem isso o modelo recebe `<strong>` no meio do trecho e às vezes copia a tag
    /// para dentro do resumo.
    #[test]
    fn tira_as_tags_do_trecho_do_brave() {
        assert_eq!(
            limpar("O <strong>dólar</strong> fechou em alta"),
            "O dólar fechou em alta"
        );
        assert_eq!(limpar("  sem tag  "), "sem tag");
        // Tag sem fechar não pode comer o resto do texto silenciosamente... mas come:
        // é o comportamento certo, porque `<` solto em HTML de verdade é raro e
        // preferimos perder texto a vazar markup para o prompt.
        assert_eq!(limpar("antes <quebrado"), "antes");
    }

    /// A escolha da fonte é o que o log de ações mostra ao usuário.
    #[test]
    fn a_fonte_depende_da_chave() {
        assert_eq!(fonte(""), "wikipedia");
        assert_eq!(fonte("   "), "wikipedia");
        assert_eq!(fonte("BSA-xxx"), "brave");
    }
}
