//! Ler a PÁGINA do resultado — o capítulo que faltava no módulo pai.
//!
//! O doc do [`super`] registra que raspar o BUSCADOR não dá: o DuckDuckGo devolve desafio
//! anti-bot, o Mojeek e as instâncias do SearXNG devolvem 403. Isso continua valendo, e
//! nada aqui tenta de novo. **O que este módulo faz é outra coisa**: quem busca continua
//! sendo a API do Google, e o que se lê é a página que ELA apontou — um site comum,
//! servido para qualquer visitante que peça.
//!
//! **Por que valeu a pena.** O `snippet` do Custom Search tem 150 a 200 caracteres, e é
//! com ele que o modelo redige a resposta. Duas linhas e meia não respondem "quanto
//! custa" nem "como funciona", e o que o modelo faz com pouco material é completar com o
//! que ele "lembra" — o modo de falha que o prompt do `converse::responder_com_busca`
//! gasta 48 linhas tentando impedir. Dar material de verdade ataca a causa; as 48 linhas
//! atacavam o sintoma.
//!
//! **Só o ramo do Google, e as manchetes de fora.** O resumo REST da Wikipédia já traz 300
//! a 600 caracteres do primeiro parágrafo do artigo, que é exatamente o texto que um
//! raspador extrairia — gastar um GET de 200 kB para chegar a uma versão pior organizada
//! do que já se tem não paga. As manchetes ficam fora por construção (o pai chama isto
//! ANTES de acrescentá-las): os links do RSS são redirecionadores do `news.google.com` que
//! caem em página de consentimento, e site de notícia é onde mais tem paywall.
//!
//! **Nada aqui pode derrubar a busca.** Mesma promessa do `noticias`: toda falha vira
//! `None` e o achado fica com o trecho que já tinha. Uma página que recusa nos deixa
//! exatamente onde estávamos antes deste módulo existir, que é um lugar conhecido.
//!
//! **A saída de emergência, se a taxa de acerto cair**: ler o texto da aba que o
//! `navegador` já abre, por script de inicialização. Não foi feita porque o tempo não
//! fecha — a aba mostra a página de RESULTADOS, e navegá-la pelos sites raspados faria a
//! janela piscar por endereços que ninguém pediu, destruindo o papel dela de mostrar de
//! onde veio a resposta — e porque criar webview de dentro de um callback é o travamento
//! documentado em `navegador.rs`. Fica registrada aqui como o DuckDuckGo ficou lá em cima.

use std::collections::HashSet;
use std::time::Duration;

use dom_query::Document;

use super::{Achado, AGENTE};

/// Quantas páginas são abertas por busca.
///
/// Duas, e não as três que a busca devolve: elas são lidas em paralelo, então o custo é o
/// da mais lenta — mas cada uma ainda ocupa espaço no prompt, e o orçamento ali é apertado
/// (ver o `num_ctx` em `converse::responder_com_busca`). Duas fontes com texto de verdade
/// respondem melhor que três com duas linhas e meia cada.
const PAGINAS: usize = 2;

/// Teto de espera por página.
///
/// Próprio, e deliberadamente menor que o `TIMEOUT` de 15 s da busca: são leituras
/// EXTRAS, penduradas num turno que já custa alguns segundos. Com 15 s aqui, um site
/// lento sozinho dobraria o pior caso — e o pior caso é o que a pessoa sente, calada,
/// esperando com o microfone fechado.
const TEMPO: Duration = Duration::from_secs(6);

/// Teto de download por página.
///
/// Lido por pedaço e cortado ao chegar aqui, em vez de olhar o `Content-Length`: resposta
/// em `chunked` não traz esse cabeçalho, e é justamente nela que mora o download sem fim.
const TETO: usize = 512 * 1024;

/// Quanto do texto extraído vira `trecho`.
///
/// O que passa daqui é cortado pelo consumidor de qualquer jeito, e sobra ocupando
/// memória e o `historico.jsonl`.
const CORTE: usize = 1_500;

/// O que nunca é conteúdo, e some antes de qualquer extração.
///
/// **É esta linha que substitui o `limpar()` do módulo pai.** Aquele foi escrito para
/// tirar `<b>` de um snippet, e o próprio teste dele registra que ele come o texto depois
/// de um `<` sem fechar. Num documento inteiro ele cuspiria o conteúdo de todo `<script>`
/// — JSON-LD e analytics — como se fosse texto da página.
const LIXO: &str =
    "script, style, noscript, template, nav, header, footer, aside, form, svg, iframe";

/// Onde o texto costuma estar, na ordem da aposta.
const RECIPIENTES: [&str; 3] = ["main", "article", "[role=main]"];

/// Os blocos que viram linha.
///
/// Extrair bloco a bloco em vez de pegar o texto do recipiente inteiro resolve um problema
/// concreto: `.text()` concatena sem separador, então o rótulo de uma célula gruda no
/// valor da seguinte e "Preço" + "R$ 3.999" chegam ao modelo como uma palavra só.
const BLOCOS: &str = "p, h1, h2, h3, h4, li, dd, td, blockquote";

/// Troca o trecho da API pelo texto da página, nos achados em que der.
pub(super) async fn enriquecer(http: &reqwest::Client, achados: &mut [Achado]) {
    let lidas = futures_util::future::join_all(
        achados
            .iter()
            .take(PAGINAS)
            .map(|achado| ler(http, &achado.url)),
    )
    .await;

    for (achado, lido) in achados.iter_mut().zip(lidas) {
        let Some(texto) = lido else { continue };

        // **Nunca piorar o que já se tem.** Uma página de consentimento, um muro de login
        // ou um artigo que veio quase todo por JavaScript rendem menos texto que o snippet
        // do Google — e trocar por eles seria pagar o GET para responder pior.
        if texto.chars().count() > achado.trecho.chars().count() {
            achado.trecho = texto;
        }
    }
}

/// Lê a página e devolve o texto dela, ou `None` quando não deu.
async fn ler(http: &reqwest::Client, url: &str) -> Option<String> {
    let resposta = http
        .get(url)
        .header("User-Agent", AGENTE)
        .timeout(TEMPO)
        .send()
        .await
        .ok()?;

    if !resposta.status().is_success() || !e_html(&resposta) {
        return None;
    }

    extrair(&String::from_utf8_lossy(&corpo(resposta).await?))
}

/// O `Content-Type` promete HTML?
///
/// Guarda barata e necessária: o Custom Search devolve PDF entre os resultados, e sem isto
/// os bytes dele virariam texto ilegível dentro do prompt.
fn e_html(resposta: &reqwest::Response) -> bool {
    resposta
        .headers()
        .get("content-type")
        .and_then(|valor| valor.to_str().ok())
        .is_some_and(|tipo| {
            let tipo = tipo.to_ascii_lowercase();
            tipo.contains("text/html") || tipo.contains("application/xhtml+xml")
        })
}

/// Baixa até [`TETO`] bytes.
async fn corpo(mut resposta: reqwest::Response) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::new();

    // Erro no meio do fluxo encerra a leitura e fica com o que já veio: meia página ainda
    // responde, e não há segunda chance a essa altura.
    while let Ok(Some(pedaco)) = resposta.chunk().await {
        bytes.extend_from_slice(&pedaco);

        if bytes.len() >= TETO {
            bytes.truncate(TETO);
            break;
        }
    }

    // **O `reqwest` deste projeto não tem a feature `gzip`**, então ele não anuncia
    // `Accept-Encoding: gzip` e o servidor responde em texto puro. Ligar a feature puxaria
    // `flate2` e `async-compression`, que é compilação nova de verdade. Este guarda existe
    // para o servidor que comprime mesmo sem ninguém pedir: sem ele, os bytes do gzip
    // virariam caracteres de substituição e daí um "texto" sem sentido no prompt.
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return None;
    }

    (!bytes.is_empty()).then_some(bytes)
}

/// Tira o texto de leitura de um documento HTML.
fn extrair(html: &str) -> Option<String> {
    let documento = Document::from(html);
    documento.select(LIXO).remove();

    let recipiente = RECIPIENTES
        .iter()
        .find_map(|alvo| documento.try_select(alvo))
        .unwrap_or_else(|| documento.select("body"));

    // Bloco aninhado dentro de bloco (um `<p>` dentro de um `<li>`) casa duas vezes e
    // entregaria o mesmo texto duas vezes, gastando o orçamento do prompt para repetir.
    let mut vistos = HashSet::new();
    let mut linhas: Vec<String> = recipiente
        .select(BLOCOS)
        .iter()
        .map(|bloco| espremer(&bloco.text()))
        .filter(|linha| !linha.is_empty() && vistos.insert(linha.clone()))
        .collect();

    // Página sem bloco nenhum — um `<div>` com texto solto, que existe mais do que devia.
    if linhas.is_empty() {
        linhas.push(espremer(&recipiente.text()));
    }

    let texto: String = linhas.join("\n").chars().take(CORTE).collect();
    (!texto.trim().is_empty()).then_some(texto)
}

/// Junta o espaço em branco do HTML numa linha só.
fn espremer(texto: &str) -> String {
    texto.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma página como as que chegam de verdade: o texto que interessa dentro de um
    /// `<article>`, cercado de menu, rodapé e um `<script>` com JSON dentro.
    ///
    /// O JSON é o ponto. Ele é o que o `limpar()` do módulo pai entregaria como se fosse
    /// texto da página, e é o que motivou trazer um parser em vez de reaproveitá-lo.
    const PAGINA: &str = r#"<!doctype html>
<html><head>
  <title>Console novo</title>
  <script type="application/ld+json">{"@type":"Product","name":"NAO_E_TEXTO"}</script>
  <style>.menu { color: red }</style>
</head>
<body>
  <nav><a href="/a">Início</a><a href="/b">Ofertas</a><a href="/c">Conta</a></nav>
  <header><h1>Loja</h1></header>
  <article>
    <h2>PlayStation 5 Slim</h2>
    <p>O console custa R$ 3.799 na loja oficial, com o leitor de disco incluso.</p>
    <ul><li>Entrega em 5 dias úteis</li></ul>
  </article>
  <div class="relacionados"><p>FORA_DO_ARTIGO: veja também o console anterior.</p></div>
  <footer><p>Todos os direitos reservados RODAPE_NAO_ENTRA</p></footer>
  <script>window.analytics = "TAMBEM_NAO";</script>
</body></html>"#;

    #[test]
    fn o_script_e_o_menu_nao_viram_texto() {
        let texto = extrair(PAGINA).expect("a página tem conteúdo");

        assert!(!texto.contains("NAO_E_TEXTO"), "o JSON-LD vazou: {texto}");
        assert!(!texto.contains("TAMBEM_NAO"), "o script vazou: {texto}");
        assert!(!texto.contains("Ofertas"), "o menu vazou: {texto}");
    }

    /// O `<article>` vence o resto da página: o rodapé está fora dele e não entra, mesmo
    /// sendo um `<p>` legítimo.
    #[test]
    fn o_recipiente_recorta_a_pagina() {
        let texto = extrair(PAGINA).expect("a página tem conteúdo");

        assert!(texto.contains("R$ 3.799"));
        assert!(texto.contains("Entrega em 5 dias úteis"));
        // O rodapé sairia pelo LIXO de qualquer jeito; o `<div>` dos relacionados é um
        // `<p>` comum, e só o recorte pelo `<article>` o mantém fora.
        assert!(!texto.contains("FORA_DO_ARTIGO"), "vazou de fora: {texto}");
        assert!(
            !texto.contains("RODAPE_NAO_ENTRA"),
            "pegou o rodapé: {texto}"
        );
    }

    /// Sem separador entre blocos, o título grudaria no preço e chegariam ao modelo como
    /// uma palavra só — que é a razão de a extração ser bloco a bloco.
    #[test]
    fn cada_bloco_vira_uma_linha() {
        let texto = extrair(PAGINA).expect("a página tem conteúdo");

        assert!(
            texto.contains("PlayStation 5 Slim\nO console custa"),
            "os blocos grudaram: {texto:?}"
        );
    }

    #[test]
    fn o_texto_e_cortado_no_teto() {
        let longa = format!("<body><p>{}</p></body>", "palavra ".repeat(1_000));
        let texto = extrair(&longa).expect("tem conteúdo");

        assert_eq!(texto.chars().count(), CORTE);
    }

    /// O corte conta CARACTERES, e não bytes: cortar no meio de um "ç" produziria uma
    /// `String` inválida — em Rust, um pânico.
    #[test]
    fn o_corte_respeita_o_acento() {
        let longa = format!("<body><p>{}</p></body>", "canção ".repeat(1_000));
        let texto = extrair(&longa).expect("tem conteúdo");

        assert_eq!(texto.chars().count(), CORTE);
    }

    /// Nada aqui pode derrubar a busca — nem com HTML que não é HTML.
    ///
    /// Página sem texto nenhum vira `None`, que é o que faz o achado ficar com o trecho
    /// que já tinha. Página quebrada NÃO vira `None`: o `html5ever` fecha as tags no
    /// lugar do autor e devolve o texto que houver, e meia página ainda responde. O que
    /// se cobra aqui é que nenhum dos dois caminhos entre em pânico.
    #[test]
    fn pagina_vazia_ou_quebrada_nao_derruba_nada() {
        assert_eq!(extrair(""), None);
        assert_eq!(extrair("<html><body></body></html>"), None);
        assert_eq!(
            extrair(
                "   
  "
            ),
            None
        );

        assert_eq!(
            extrair("<p>sem fechar <div><span>").as_deref(),
            Some("sem fechar")
        );
    }

    /// Página sem bloco nenhum, só texto solto num `<div>`. Acontece, e o caminho de
    /// escape tem que devolver alguma coisa em vez de vazio.
    #[test]
    fn texto_solto_fora_de_bloco_ainda_e_lido() {
        let texto = extrair("<body><div>o preço subiu ontem</div></body>")
            .expect("tem texto, mesmo sem bloco");

        assert_eq!(texto, "o preço subiu ontem");
    }

    #[test]
    fn espremer_junta_o_espaco_do_html() {
        assert_eq!(
            espremer("  duas\n\n  linhas\t e   um tab "),
            "duas linhas e um tab"
        );
    }
}
