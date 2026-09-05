//! Busca na web, para o Jarvis responder o que não está na memória dele.
//!
//! **Três fontes, e cada escolha foi medida.** O que eu queria era raspar um buscador e
//! não depender de chave nenhuma. Não dá:
//!
//! | fonte | resultado |
//! | --- | --- |
//! | `html.duckduckgo.com` | página de desafio anti-bot (`anomaly`, `challenge-form`) |
//! | `lite.duckduckgo.com` | o mesmo desafio, HTTP 202 — a versão "leve" não é mais aberta |
//! | `www.mojeek.com` | 403 |
//! | `api.duckduckgo.com` (Instant Answer) | `AbstractText` vazio em 4 de 4 consultas reais |
//! | instâncias públicas de SearXNG | JSON desligado, 403 ou "Too Many Requests" |
//! | **Wikipedia** | **3 acertos em 4** — falha só no que não é enciclopédico |
//! | **Google News (RSS)** | **200, sem chave, 100 itens datados** |
//!
//! Então: **Wikipedia + manchetes por padrão**, sem configurar nada, e uma chave do
//! Google nas configurações que troca a Wikipedia por busca web de verdade.
//!
//! **O Brave Search saiu.** Ele era a segunda fonte com chave, escolhido pela cota de
//! 2.000 buscas/mês contra as 100/dia do Google — mas o plano "gratuito" dele pede
//! cartão de crédito no cadastro, e uma fonte que exige cartão é, para este app, o
//! mesmo que uma fonte paga. Duas fontes com chave também dobravam a superfície de
//! configuração por metade do ganho: as duas respondem web de verdade, e é isso que a
//! Wikipedia não faz.
//!
//! **Por que as manchetes entraram.** A Wikipedia é enciclopédia, e enciclopédia não tem
//! HOJE: perguntado "quanto está a Bitcoin?", ela devolveu os verbetes *Bitcoin*, *Mercado
//! Bitcoin* e *Mineração de Bitcoin*, e o Jarvis respondeu explicando o que é a moeda —
//! que foi exatamente a reclamação que originou esta fonte. O RSS do Google News responde
//! a mesma pergunta com "Preço do Bitcoin fica em US$ 77 mil…", **com data**. Não é busca
//! web (não há trecho da página, só a manchete), mas é fato atual e datado, que é o que
//! faltava.
//!
//! O RSS e não uma API: não existe API pública do Google News, e o feed responde 200 sem
//! chave, sem cabeçalho especial e sem desafio — os três tropeços da tabela acima.
//!
//! ponytail: sem cache. Perguntar duas vezes bate duas vezes na rede. Um mapa com TTL
//! entra quando (e se) isso incomodar.
//!
//! **E raspar a PÁGINA do resultado dá certo** — não confunda com a tabela lá em cima.
//! O que não deu foi raspar o BUSCADOR, que se defende de robô por profissão; a página que
//! ele apontou é um site comum, servido para quem pedir. Quem faz isso é o [`pagina`], e
//! ele existe porque o `snippet` do Custom Search tem 150 caracteres — material curto
//! demais para o modelo responder sem completar de cabeça.

mod pagina;

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
    /// O buscador respondeu, e disse não.
    ///
    /// **Carrega o `motivo` porque o número sozinho não conserta nada.** Um 403 do Google
    /// tem pelo menos quatro causas — API não habilitada no projeto, chave restrita por IP
    /// ou referrer, chave de outro projeto, `cx` inválido — e todas viravam a mesma linha
    /// na tela. O corpo da resposta dele já traz qual delas é, em texto; jogá-lo fora era
    /// transformar um erro que se resolve em cinco minutos num que se resolve por
    /// tentativa e erro.
    #[error("a busca recusou a chamada (HTTP {status}): {motivo}")]
    Recusada { status: u16, motivo: String },
}

/// As credenciais que decidem ONDE ele pesquisa.
///
/// Uma struct em vez de três `&str` soltos porque a escolha da fonte é uma regra, não uma
/// lista de parâmetros: quem chama passa o que tem, e o [`Chaves::escolher`] é o único
/// lugar que sabe qual ganha de qual.
#[derive(Debug, Clone, Copy)]
pub struct Chaves<'a> {
    pub google: &'a str,
    /// O mecanismo de pesquisa programável do Google. Sem ele a chave não serve para nada.
    pub google_cx: &'a str,
}

/// Quem vai responder a busca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fonte<'a> {
    Google { chave: &'a str, cx: &'a str },
    Wikipedia,
}

impl<'a> Chaves<'a> {
    /// **Google quando as duas credenciais existem; Wikipédia quando não.**
    ///
    /// Duas fontes e não três: o Google é o índice que responde pergunta de compra, e a
    /// Wikipédia é o que faz o app funcionar sem nenhum cadastro. Não há meio-termo útil
    /// entre as duas — o que existia (Brave) pedia cartão de crédito pela cota grátis.
    ///
    /// Chave sem `cx` cai para a Wikipédia em vez de quebrar: é o erro de meia-configuração
    /// mais provável, e derrubar a busca por causa dele seria pior que responder pior.
    pub fn escolher(&self) -> Fonte<'a> {
        let google = self.google.trim();
        let cx = self.google_cx.trim();
        if !google.is_empty() && !cx.is_empty() {
            return Fonte::Google { chave: google, cx };
        }

        Fonte::Wikipedia
    }

    /// O nome que vai para o log de ações, para a linha BUSCA dizer de onde veio.
    pub fn nome_da_fonte(&self) -> &'static str {
        match self.escolher() {
            Fonte::Google { .. } => "google + notícias",
            Fonte::Wikipedia => "wikipedia + notícias",
        }
    }
}

/// Um resultado, já limpo o suficiente para ir ao prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    pub titulo: String,
    pub trecho: String,
    pub url: String,
    /// A data, quando o achado é uma NOTÍCIA. `None` no que é enciclopédico ou de página.
    ///
    /// Serve a duas coisas, e as duas importam. Na resposta, ela é a diferença entre "a
    /// Bitcoin está em US$ 77 mil" e "estava em US$ 77 mil na sexta" — sem a data, o
    /// modelo fala de ontem no presente. Na memória, ela é o que impede a manchete de
    /// virar nota: notícia é retrato de um dia, e nota é o que continua valendo amanhã.
    pub quando: Option<String>,
}

/// A Wikipedia recusa chamada sem User-Agent identificável.
const AGENTE: &str = "Jarvis/0.1 (assistente pessoal local)";

/// Busca, preferindo o Google quando há chave e `cx`.
pub async fn pesquisar(
    http: &reqwest::Client,
    consulta: &str,
    chaves: &Chaves<'_>,
) -> Result<Vec<Achado>, SearchError> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return Err(SearchError::SemConsulta);
    }

    // **A ordem é a da qualidade, e a Wikipédia é o último recurso de propósito.** Ela
    // responde bem "quem foi X" e "o que é Y", e responde MAL tudo que muda: preço,
    // estoque, promoção, resultado de ontem. Quando existe um buscador de verdade
    // configurado, ela sai inteira do caminho — não entra como reforço, porque três
    // verbetes sobre o objeto empurram para fora os poucos resultados que tinham a
    // resposta.
    let principal = match chaves.escolher() {
        Fonte::Google { chave, cx } => google(http, consulta, chave, cx).await,
        Fonte::Wikipedia => wikipedia(http, consulta).await,
    };

    // A chave recusada TEM que aparecer: é configuração errada, e o usuário é o único que
    // pode consertar. Qualquer outra falha da fonte principal só vira erro se as manchetes
    // também vierem vazias — derrubar a busca por causa da Wikipedia fora do ar seria
    // trocar meia resposta por nenhuma.
    if let Err(SearchError::Recusada { status, motivo }) = &principal {
        return Err(SearchError::Recusada {
            status: *status,
            motivo: motivo.clone(),
        });
    }

    let mut achados = match &principal {
        Ok(itens) => itens.clone(),
        Err(_) => Vec::new(),
    };

    // **Só o Google, e só ANTES das manchetes.** O snippet dele é de 150 caracteres e
    // pede a página inteira para virar resposta; o resumo REST da Wikipédia logo abaixo já
    // é o primeiro parágrafo do artigo, e não tem o que melhorar. As manchetes entram
    // depois desta linha justamente para ficarem de fora: o link do RSS é um
    // redirecionador que cai em página de consentimento. Os porquês completos moram no
    // doc do `pagina`.
    if matches!(chaves.escolher(), Fonte::Google { .. }) {
        pagina::enriquecer(http, &mut achados).await;
    }

    achados.extend(noticias(http, consulta).await);

    if achados.is_empty() {
        // O erro da fonte principal explica melhor que "não achei nada": ele diz se foi
        // rede, recusa, ou busca que rodou e voltou vazia.
        return match principal {
            Ok(_) => Err(SearchError::NadaEncontrado(consulta.to_owned())),
            Err(erro) => Err(erro),
        };
    }

    Ok(achados)
}

/// Nome das fontes, para o log de ações dizer de onde veio a resposta.
///
/// Diz o que foi CONSULTADO, não o que respondeu: as manchetes entram sempre, e quando o
/// feed não responde a linha do log continua verdadeira — ela conta onde ele foi olhar.
pub fn fonte(chaves: &Chaves<'_>) -> &'static str {
    chaves.nome_da_fonte()
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
        quando: None,
        url: resumo
            .content_urls
            .map(|enderecos| enderecos.desktop.page)
            .unwrap_or_default(),
    })
}

// ---- Google News (RSS) -------------------------------------------------------

/// Quantas manchetes entram junto dos resultados principais.
///
/// Menos que os outros de propósito: manchete é uma linha, e o que ela traz de útil é o
/// FATO e a DATA. Três cobrem "o que está acontecendo com X" sem empurrar os verbetes para
/// fora da janela de um modelo de 3B.
const MANCHETES: usize = 3;

/// As notícias recentes sobre o assunto, com data. **Nunca falha a busca**: feed fora do
/// ar devolve lista vazia, e o resto da resposta continua de pé.
async fn noticias(http: &reqwest::Client, consulta: &str) -> Vec<Achado> {
    let url = format!(
        "https://news.google.com/rss/search?q={}&hl=pt-BR&gl=BR&ceid=BR:pt-419",
        urlencode(consulta)
    );

    let resposta = http
        .get(&url)
        .timeout(TIMEOUT)
        .header("User-Agent", AGENTE)
        .send()
        .await;

    let Ok(resposta) = resposta else {
        return Vec::new();
    };
    if !resposta.status().is_success() {
        return Vec::new();
    }
    let Ok(xml) = resposta.text().await else {
        return Vec::new();
    };

    manchetes(&xml)
}

/// O XML vira achados. Separado da rede para poder ser testado com um feed de mentira —
/// é a única parte disto que pode quebrar em silêncio quando o Google mudar o formato.
fn manchetes(xml: &str) -> Vec<Achado> {
    xml.split("<item>")
        .skip(1)
        .filter_map(|item| {
            let titulo = sem_entidades(etiqueta(item, "title")?);
            if titulo.trim().is_empty() {
                return None;
            }

            let quando = etiqueta(item, "pubDate").map(data_curta);

            Some(Achado {
                // A manchete JÁ diz a fonte ("… - Portal do Bitcoin"), então o trecho não
                // precisa repeti-la. Ele carrega o que a manchete não tem: quando foi.
                trecho: match &quando {
                    Some(dia) => format!("Manchete de {dia}."),
                    None => "Manchete recente.".to_owned(),
                },
                titulo,
                url: etiqueta(item, "link")
                    .map(str::to_owned)
                    .unwrap_or_default(),
                quando,
            })
        })
        .take(MANCHETES)
        .collect()
}

/// O conteúdo de `<etiqueta>…</etiqueta>`, cru.
///
/// Um parser de XML inteiro seria uma crate a mais por três campos de um feed que este
/// projeto não controla — e que, quando mudar, muda de um jeito que nenhum parser
/// salvaria. O `manchetes` tem teste justamente por isso.
fn etiqueta<'a>(item: &'a str, nome: &str) -> Option<&'a str> {
    let abre = item.find(&format!("<{nome}>"))? + nome.len() + 2;
    let fecha = item[abre..].find(&format!("</{nome}>"))? + abre;

    Some(item[abre..fecha].trim())
}

/// `Sat, 29 Aug 2026 16:03:00 GMT` vira `29/08/2026`.
///
/// Só o dia: hora de publicação de notícia não muda resposta nenhuma, e a data é o que
/// separa "está" de "estava".
fn data_curta(rfc2822: &str) -> String {
    match chrono::DateTime::parse_from_rfc2822(rfc2822) {
        Ok(data) => data.format("%d/%m/%Y").to_string(),
        // Formato inesperado vira o texto cru: uma data feia informa mais que nenhuma.
        Err(_) => rfc2822.to_owned(),
    }
}

/// As cinco entidades que aparecem em manchete, e as numéricas decimais.
///
/// `&#39;` em "o &#39;rali&#39; do bitcoin" atravessa o prompt e sai na fala do Jarvis
/// como está — é pequeno e é exatamente o tipo de sujeira que ninguém liga à causa.
fn sem_entidades(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;

    while let Some(inicio) = resto.find('&') {
        saida.push_str(&resto[..inicio]);
        let daqui = &resto[inicio..];

        let Some(fim) = daqui.find(';').filter(|fim| *fim <= 8) else {
            saida.push('&');
            resto = &daqui[1..];
            continue;
        };

        let entidade = &daqui[1..fim];
        match entidade {
            "amp" => saida.push('&'),
            "lt" => saida.push('<'),
            "gt" => saida.push('>'),
            "quot" => saida.push('"'),
            "apos" => saida.push('\''),
            numerica if numerica.starts_with('#') => {
                match numerica[1..].parse::<u32>().ok().and_then(char::from_u32) {
                    Some(letra) => saida.push(letra),
                    None => saida.push_str(&daqui[..=fim]),
                }
            }
            _ => saida.push_str(&daqui[..=fim]),
        }

        resto = &daqui[fim + 1..];
    }

    saida.push_str(resto);
    saida
}

// ---- Google ------------------------------------------------------------------

/// Busca no índice do Google, pela Custom Search JSON API.
///
/// **Entrou porque a Wikipédia não é um buscador.** Perguntado "preço PlayStation 5", o
/// `wikipedia` devolveu os verbetes de *PlayStation 5*, *PlayStation* e *PlayStation 3* —
/// a palavra "preço" foi ignorada, porque uma enciclopédia indexa o OBJETO, não quanto
/// ele custa. Preço, promoção, onde comprar, quem ganhou ontem: nada disso existe lá, e
/// insistir só entrega ao modelo três textos sobre a coisa errada.
///
/// Precisa de DUAS credenciais, e é a única fonte daqui que precisa: a chave da API e o
/// `cx`, que identifica o mecanismo de pesquisa programável. O `cx` não é opcional e não
/// tem padrão — ele é onde se configura "buscar na web inteira" em vez de num site só.
async fn google(
    http: &reqwest::Client,
    consulta: &str,
    chave: &str,
    cx: &str,
) -> Result<Vec<Achado>, SearchError> {
    #[derive(Deserialize)]
    struct Resposta {
        #[serde(default)]
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        title: String,
        #[serde(default)]
        snippet: String,
        link: String,
    }

    let url = format!(
        "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={QUANTOS}&hl=pt&gl=br",
        urlencode(chave),
        urlencode(cx),
        urlencode(consulta)
    );

    let resposta = http
        .get(&url)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        // O corpo do erro do Google é JSON com `error.message` em inglês, e é a única
        // coisa que diz QUAL das causas foi. Ler antes de desistir é o que separa
        // "recusou (403)" de "a Custom Search API não está habilitada neste projeto".
        let corpo = resposta.text().await.unwrap_or_default();
        return Err(SearchError::Recusada {
            status: status.as_u16(),
            motivo: motivo_do_google(&corpo, status.as_u16()),
        });
    }

    let corpo: Resposta = resposta
        .json()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))?;

    Ok(corpo
        .items
        .into_iter()
        .take(QUANTOS)
        .map(|item| Achado {
            titulo: item.title,
            trecho: limpar(&item.snippet),
            url: item.link,
            // Resultado de web não é notícia: sem data, o `nota_da_busca` o considera
            // conhecimento que continua valendo, que é o certo para uma página.
            quando: None,
        })
        .collect())
}

/// O `error.message` do Google, traduzido para o que a pessoa tem que FAZER.
///
/// As três causas cobertas são as que aparecem de verdade ao configurar isto pela primeira
/// vez, e cada uma tem uma ação diferente — que é justamente o que o número do HTTP não
/// diz. O que não casar volta como veio: a mensagem crua em inglês ainda é melhor que
/// "recusou".
fn motivo_do_google(corpo: &str, status: u16) -> String {
    let cru = corpo.to_lowercase();

    // Três redações para a MESMA causa, e a terceira custou um 403 despejado como JSON cru
    // na bolha da conversa: o Google responde "has not been used", "accessNotConfigured" ou
    // "does not have the access to Custom Search JSON API" conforme o caminho, e só as duas
    // primeiras estavam aqui.
    if cru.contains("has not been used")
        || cru.contains("accessnotconfigured")
        || cru.contains("does not have the access")
    {
        return "a Custom Search API não está habilitada neste projeto do Google Cloud. \
                Abra console.cloud.google.com, procure \"Custom Search API\" e clique em \
                ATIVAR — leva um minuto e vale para a chave que você já criou"
            .to_owned();
    }

    if cru.contains("referer") || cru.contains("referrer") || cru.contains("ip address") {
        return "a chave está restrita por site ou por IP, e este app chama direto do seu \
                computador. No console de credenciais, deixe a restrição da chave como \
                \"Nenhuma\" ou limite por API em vez de por origem"
            .to_owned();
    }

    if status == 429 || cru.contains("quota") || cru.contains("rate limit") {
        return "a cota diária de 100 buscas do plano gratuito acabou. Ela reseta à \
                meia-noite no fuso do Pacífico"
            .to_owned();
    }

    // Sem casar com nada conhecido, devolve o que o Google disse — cortado, porque isto vai
    // para uma bolha de conversa e o corpo pode vir com o JSON inteiro.
    let limpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpo.is_empty() {
        return "a fonte não explicou".to_owned();
    }
    limpo.chars().take(220).collect()
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
            motivo: "a fonte não explicou".to_owned(),
        });
    }

    resposta
        .json()
        .await
        .map_err(|erro| SearchError::Rede(erro.to_string()))
}

/// Buscador devolve marcação no trecho para destacar o termo casado. Tirar tag é mais
/// barato que ensinar o modelo a ignorá-la — e ela chegaria à FALA, onde "abre colchete
/// strong" é o mesmo tipo de ruído que o `tirar_links_orfaos` já combate na conversa.
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
    fn tira_as_tags_do_trecho_do_buscador() {
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

    fn chaves<'a>(google: &'a str, cx: &'a str) -> Chaves<'a> {
        Chaves {
            google,
            google_cx: cx,
        }
    }

    /// O 403 do Google tem quatro causas e uma ação diferente para cada. O teste guarda
    /// a tradução — sem ela a tela mostrava só o número, que não conserta nada.
    #[test]
    fn o_403_do_google_vira_o_que_fazer() {
        let nao_habilitada = r#"{"error":{"code":403,"message":"Custom Search API has not been used in project 123 before or it is disabled."}}"#;
        assert!(motivo_do_google(nao_habilitada, 403).contains("ATIVAR"));

        // A MESMA causa, na terceira redação que o Google usa — e a que escapava. Ela caía
        // no fim da função e o JSON cru ia inteiro para a bolha da conversa, onde a pessoa
        // lia um erro em inglês em vez do "clique em ATIVAR" que estava a três linhas dali.
        let sem_acesso = r#"{"error":{"code":403,"message":"This project does not have the access to Custom Search JSON API."}}"#;
        assert!(motivo_do_google(sem_acesso, 403).contains("ATIVAR"));

        let restrita = r#"{"error":{"message":"Requests from referer <empty> are blocked."}}"#;
        assert!(motivo_do_google(restrita, 403).contains("restrita"));

        assert!(motivo_do_google("{}", 429).contains("cota"));

        // O que não casa volta como veio: inglês cru ainda diz mais que "recusou".
        let estranho = motivo_do_google(r#"{"error":"algo novo que ninguem previu"}"#, 400);
        assert!(estranho.contains("algo novo"), "{estranho}");

        assert_eq!(motivo_do_google("", 500), "a fonte não explicou");
    }

    /// A escolha da fonte é o que o log de ações mostra ao usuário.
    #[test]
    fn a_fonte_depende_da_chave() {
        assert_eq!(fonte(&chaves("", "")), "wikipedia + notícias");
        assert_eq!(fonte(&chaves("  ", " ")), "wikipedia + notícias");
        assert_eq!(fonte(&chaves("AIza-xxx", "cx1")), "google + notícias");
    }

    /// **O Google ganha da Wikipédia, e precisa das DUAS credenciais para ganhar.**
    ///
    /// A preferência não é gosto: foi ela que consertou "qual o preço do PlayStation 5",
    /// que a Wikipédia respondia com os verbetes de PlayStation 5, PlayStation e
    /// PlayStation 3 — nenhum deles com preço.
    #[test]
    fn o_google_ganha_da_wikipedia_mas_so_com_as_duas_credenciais() {
        assert_eq!(
            chaves("AIza-xxx", "cx1").escolher(),
            Fonte::Google {
                chave: "AIza-xxx",
                cx: "cx1"
            }
        );
        // Chave sem `cx` não busca nada, e `cx` sem chave também não: os dois caem para a
        // Wikipédia em vez de derrubar a busca. É o erro de quem configura pela metade.
        assert_eq!(chaves("AIza-xxx", "").escolher(), Fonte::Wikipedia);
        assert_eq!(chaves("", "cx1").escolher(), Fonte::Wikipedia);
    }

    /// Um feed do Google News encurtado, com as três coisas que importam: manchete com
    /// entidade, data em RFC 2822, e um item sem data.
    const FEED: &str = r#"<rss version="2.0"><channel>
<title>bitcoin - Google Notícias</title>
<item>
<title>Preço do Bitcoin fica em US$ 77 mil após ETFs perderem US$ 202 milhões - Economic News</title>
<link>https://news.google.com/rss/articles/abc</link>
<pubDate>Sat, 29 Aug 2026 16:03:00 GMT</pubDate>
</item>
<item>
<title>O &#39;rali&#39; do bitcoin &amp; o discurso do Fed - Portal do Bitcoin</title>
<link>https://news.google.com/rss/articles/def</link>
<pubDate>Fri, 28 Aug 2026 12:06:00 GMT</pubDate>
</item>
<item>
<title>Sem data nenhuma - Jornal</title>
<link>https://news.google.com/rss/articles/ghi</link>
</item>
<item>
<title>O quarto item, que não deve entrar - Jornal</title>
<link>https://news.google.com/rss/articles/jkl</link>
<pubDate>Thu, 27 Aug 2026 09:00:00 GMT</pubDate>
</item>
</channel></rss>"#;

    /// **O contrato com um XML que este projeto não controla.** O Google muda o feed sem
    /// avisar, e o sintoma seria a busca voltar a responder só com verbete — sem erro, sem
    /// log, sem nada que ligue uma coisa à outra.
    #[test]
    fn o_feed_de_noticias_vira_achados_com_data() {
        let achados = manchetes(FEED);

        assert_eq!(achados.len(), MANCHETES, "o teto de manchetes vale");

        assert_eq!(
            achados[0].titulo,
            "Preço do Bitcoin fica em US$ 77 mil após ETFs perderem US$ 202 milhões - Economic News"
        );
        assert_eq!(achados[0].quando.as_deref(), Some("29/08/2026"));
        // A data também vai no trecho: é ele que o modelo lê.
        assert_eq!(achados[0].trecho, "Manchete de 29/08/2026.");
        assert_eq!(achados[0].url, "https://news.google.com/rss/articles/abc");

        // As entidades saem — `&#39;` na fala do Jarvis é sujeira que ninguém liga à causa.
        assert_eq!(
            achados[1].titulo,
            "O 'rali' do bitcoin & o discurso do Fed - Portal do Bitcoin"
        );

        // Item sem `pubDate` entra mesmo assim: a manchete ainda informa, e sem data ela
        // só não pode ser citada como sendo de hoje.
        assert_eq!(achados[2].quando, None);
        assert_eq!(achados[2].trecho, "Manchete recente.");
    }

    /// O título do canal também é um `<title>`, mas fora de `<item>` — se ele entrasse, a
    /// primeira "notícia" de toda busca seria "bitcoin - Google Notícias".
    #[test]
    fn o_titulo_do_canal_nao_vira_noticia() {
        let achados = manchetes(FEED);

        assert!(
            !achados
                .iter()
                .any(|achado| achado.titulo.contains("Google Notícias")),
            "o cabeçalho do feed vazou para os achados"
        );
    }

    #[test]
    fn feed_vazio_ou_quebrado_nao_derruba_a_busca() {
        assert!(manchetes("").is_empty());
        assert!(manchetes("<rss><channel></channel></rss>").is_empty());
        // Item pela metade: o que dá para ler, lê; o que não dá, ignora.
        assert!(manchetes("<item><title></title></item>").is_empty());
    }

    #[test]
    fn a_data_vira_dia_legivel() {
        assert_eq!(data_curta("Sat, 29 Aug 2026 16:03:00 GMT"), "29/08/2026");
        // Formato que não bate volta cru: data feia informa mais que data nenhuma.
        assert_eq!(data_curta("ontem"), "ontem");
    }

    #[test]
    fn as_entidades_da_manchete_viram_texto() {
        assert_eq!(sem_entidades("a &amp; b"), "a & b");
        assert_eq!(sem_entidades("o &#39;rali&#39;"), "o 'rali'");
        assert_eq!(sem_entidades("&quot;alta&quot;"), "\"alta\"");
        // `&` solto é comum em manchete e não pode comer o resto do texto.
        assert_eq!(sem_entidades("Fed & juros hoje"), "Fed & juros hoje");
        // Entidade que não conhecemos passa inteira, em vez de sumir.
        assert_eq!(sem_entidades("50&nbsp;mil"), "50&nbsp;mil");
    }
}
