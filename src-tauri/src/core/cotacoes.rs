//! Dólar, euro e criptomoedas, pela AwesomeAPI.
//!
//! **Sem chave de API**, pela mesma razão do [`crate::core::tempo`]: uma feature que exige
//! cadastro é uma feature que a maioria nunca liga.
//!
//! Uma rota só, e é ela que decide o desenho todo:
//! `economia.awesomeapi.com.br/last/USD-BRL,BTC-BRL,ETH-BRL` devolve as três de uma vez.
//! Por isso não existe "buscar uma moeda" aqui — buscar TODAS custa o mesmo que buscar
//! uma, e é o que permite a pergunta "quanto tá o dólar?" alimentar o card inteiro na
//! tela sem uma segunda ida à rede.
//!
//! **Por que não a CoinGecko ou a Binance.** As duas dão cripto e nenhuma dá o dólar
//! comercial em real — precisaria de duas integrações e de somar preços de fontes com
//! horários diferentes, que é como se produz um número que não bate com nenhum lugar. A
//! AwesomeAPI cota fiat e cripto no MESMO endpoint e já em BRL, que é a moeda em que a
//! pergunta é feita.

use std::time::Duration;

use serde::Deserialize;

/// Curto de propósito: isto entra na espera de uma conversa, como no `tempo`.
const TIMEOUT: Duration = Duration::from_secs(8);

const ROTA: &str = "https://economia.awesomeapi.com.br/last";

/// O que o card mostra e o que a resposta falada usa.
///
/// **São os quatro pares que cabem numa frase.** A API aceita dezenas, mas uma resposta
/// que lista oito moedas não é resposta, é relatório — e o card vira uma tabela que
/// ninguém lê de relance.
const PARES: &str = "USD-BRL,EUR-BRL,BTC-BRL,ETH-BRL";

#[derive(Debug, thiserror::Error)]
pub enum CotacaoError {
    #[error("não consegui falar com o serviço de cotações: {0}")]
    Rede(String),
    #[error("o serviço de cotações recusou a consulta (HTTP {0})")]
    Recusada(u16),
    #[error("o serviço de cotações respondeu algo que não entendi")]
    Corpo,
}

/// Quais moedas o usuário pode pedir pelo nome.
///
/// **Enum FECHADO, e é isso que o torna seguro no schema do roteador.** É a mesma lição
/// do `direcao` do `camera_move`: campo de texto livre o modelo inventa ("moeda: peso
/// argentino"), enum fechado a gramática do llama.cpp não deixa emitir. Foi o que evitou
/// aqui os quatro verbos separados que o `weather`/`weather_at` precisou ter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Moeda {
    Dolar,
    Euro,
    Bitcoin,
    Ethereum,
    /// "como estão as moedas?", "e as cotações?" — o resumo, que é também o padrão
    /// quando ele não nomeia nenhuma.
    Todas,
}

impl Moeda {
    /// O código do par na API. `Todas` não tem um — quem a trata é o [`resumo`].
    fn codigo(self) -> Option<&'static str> {
        match self {
            Moeda::Dolar => Some("USD"),
            Moeda::Euro => Some("EUR"),
            Moeda::Bitcoin => Some("BTC"),
            Moeda::Ethereum => Some("ETH"),
            Moeda::Todas => None,
        }
    }
}

/// Uma cotação, já pronta para virar frase ou card.
///
/// Serializada em camelCase e espelhada em `src/types/cotacoes.ts` — é o mesmo contrato
/// que as outras janelinhas usam.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cotacao {
    /// `USD`, `BTC` — serve de chave estável para a UI escolher ícone e ordem.
    pub codigo: String,
    /// "Dólar Americano/Real Brasileiro", como a API manda.
    pub nome: String,
    /// O preço de COMPRA (`bid`). É o número que aparece quando alguém pergunta "quanto
    /// está o dólar" — o `ask` é o de venda, e mostrar os dois numa conversa falada só
    /// levanta a pergunta de qual dos dois vale.
    pub valor: f64,
    /// Variação percentual do dia. **Com sinal**, porque é ela que dá a cor do card e a
    /// diferença entre "subiu" e "caiu" na fala.
    pub variacao: f64,
    pub minima: f64,
    pub maxima: f64,
    /// `YYYY-MM-DD HH:MM:SS`, como veio. Sem isso o card mostra um número sem idade, e
    /// cotação sem hora é a mesma armadilha da manchete sem data no `search`.
    pub quando: String,
}

impl Cotacao {
    /// "R$ 5,10" para fiat e "R$ 413.883" para cripto.
    ///
    /// As casas mudam com a GRANDEZA, não com o tipo da moeda: centavo importa quando o
    /// número é 5, e é ruído quando ele é 413 mil. Uma regra só cobre os quatro pares e
    /// continua valendo se um quinto entrar.
    pub fn formatado(&self) -> String {
        if self.valor >= 1000.0 {
            format!("R$ {:.0}", self.valor)
        } else {
            format!("R$ {:.2}", self.valor).replace('.', ",")
        }
    }

    /// "subiu 4,64%" / "caiu 0,32%" / "estável" — o pedaço que vira fala.
    pub fn movimento(&self) -> String {
        // Abaixo de 0,05% arredondaria para "0,0%", e dizer "subiu 0,0 por cento" é pior
        // que não dizer nada.
        if self.variacao.abs() < 0.05 {
            return "estável".to_owned();
        }

        let verbo = if self.variacao > 0.0 { "subiu" } else { "caiu" };
        format!("{verbo} {:.2}%", self.variacao.abs()).replace('.', ",")
    }
}

/// O corpo da API: um mapa `"USDBRL" -> {...}`, com TODOS os números como STRING.
///
/// As aspas não são descuido deles e não dá para pedir sem: a API devolve `"bid":"5.1001"`.
/// Por isso o parse é em dois passos — serde lê texto, e o [`numero`] converte.
#[derive(Deserialize)]
struct Bruta {
    code: String,
    name: String,
    bid: String,
    #[serde(rename = "pctChange")]
    pct_change: String,
    low: String,
    high: String,
    create_date: String,
}

/// Todas as cotações de uma vez.
///
/// **Uma chamada só para os quatro pares.** Buscar uma moeda custaria o mesmo que buscar
/// as quatro, então quem pergunta o dólar já paga o card inteiro — e o card abre sem uma
/// segunda ida à rede.
pub async fn cotacoes(http: &reqwest::Client) -> Result<Vec<Cotacao>, CotacaoError> {
    let url = format!("{ROTA}/{PARES}");

    let resposta = http
        .get(&url)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|erro| CotacaoError::Rede(erro.to_string()))?;

    if !resposta.status().is_success() {
        return Err(CotacaoError::Recusada(resposta.status().as_u16()));
    }

    let brutas: std::collections::HashMap<String, Bruta> =
        resposta.json().await.map_err(|_| CotacaoError::Corpo)?;

    // A ordem do JSON de um mapa não é estável, e a do card precisa ser: dólar, euro,
    // bitcoin, ethereum, sempre. Ordem que dança entre uma abertura e outra parece bug.
    let mut achadas: Vec<Cotacao> = Vec::new();
    for par in PARES.split(',') {
        let chave = par.replace('-', "");
        if let Some(bruta) = brutas.get(&chave) {
            achadas.push(converter(bruta));
        }
    }

    if achadas.is_empty() {
        return Err(CotacaoError::Corpo);
    }

    Ok(achadas)
}

fn converter(bruta: &Bruta) -> Cotacao {
    Cotacao {
        codigo: bruta.code.clone(),
        nome: bruta.name.clone(),
        valor: numero(&bruta.bid),
        variacao: numero(&bruta.pct_change),
        minima: numero(&bruta.low),
        maxima: numero(&bruta.high),
        quando: bruta.create_date.clone(),
    }
}

/// Texto da API vira número, e o que não virar vale zero.
///
/// Zero em vez de erro porque uma moeda ilegível não pode derrubar as outras três: o card
/// com o dólar certo e o ethereum zerado ainda serve, e um `Err` aqui apagaria a resposta
/// inteira por causa de um campo.
fn numero(texto: &str) -> f64 {
    texto.parse().unwrap_or(0.0)
}

/// A frase que ele fala, a partir do que foi perguntado.
///
/// **Uma moeda vira uma frase; `Todas` vira um resumo de duas.** É a mesma regra de
/// tamanho que o `prompt_de_conversa` aplica na conversa — quem pergunta "quanto tá o
/// dólar?" quer um número, não um boletim econômico.
pub fn resumo(cotacoes: &[Cotacao], moeda: Moeda) -> String {
    if let Some(codigo) = moeda.codigo() {
        return match cotacoes.iter().find(|c| c.codigo == codigo) {
            Some(cotacao) => format!(
                "{} está {}, {} hoje.",
                nome_curto(&cotacao.codigo),
                cotacao.formatado(),
                cotacao.movimento()
            ),
            // O par sumiu da resposta da API. Não deveria acontecer, e se acontecer é
            // melhor dizer isso do que inventar um número.
            None => "não consegui pegar essa cotação agora.".to_owned(),
        };
    }

    let linhas: Vec<String> = cotacoes
        .iter()
        .map(|cotacao| {
            format!(
                "{} {}, {}",
                nome_curto(&cotacao.codigo),
                cotacao.formatado(),
                cotacao.movimento()
            )
        })
        .collect();

    format!("Agora: {}.", linhas.join("; "))
}

/// "Dólar Americano/Real Brasileiro" é o nome da API, e ninguém fala assim.
fn nome_curto(codigo: &str) -> &str {
    match codigo {
        "USD" => "O dólar",
        "EUR" => "O euro",
        "BTC" => "O bitcoin",
        "ETH" => "O ethereum",
        outro => outro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cotacao(codigo: &str, valor: f64, variacao: f64) -> Cotacao {
        Cotacao {
            codigo: codigo.to_owned(),
            nome: String::new(),
            valor,
            variacao,
            minima: 0.0,
            maxima: 0.0,
            quando: "2026-09-03 22:40:57".to_owned(),
        }
    }

    /// As casas seguem a GRANDEZA, não o tipo da moeda — centavo importa em 5 reais e é
    /// ruído em 413 mil.
    #[test]
    fn o_formato_muda_com_o_tamanho_do_numero() {
        assert_eq!(cotacao("USD", 5.1001, 0.0).formatado(), "R$ 5,10");
        assert_eq!(cotacao("BTC", 413883.0, 0.0).formatado(), "R$ 413883");
    }

    /// Variação minúscula vira "estável": "subiu 0,0 por cento" é pior que não falar.
    #[test]
    fn variacao_perto_de_zero_nao_vira_subiu_zero() {
        assert_eq!(cotacao("USD", 5.1, 0.02).movimento(), "estável");
        assert_eq!(cotacao("USD", 5.1, -0.01).movimento(), "estável");
        assert_eq!(cotacao("BTC", 413883.0, 4.636).movimento(), "subiu 4,64%");
        assert_eq!(cotacao("USD", 5.1, -0.32).movimento(), "caiu 0,32%");
    }

    #[test]
    fn uma_moeda_vira_uma_frase_e_todas_viram_o_resumo() {
        let cotacoes = vec![
            cotacao("USD", 5.1001, 0.135),
            cotacao("BTC", 413883.0, 4.636),
        ];

        assert_eq!(
            resumo(&cotacoes, Moeda::Dolar),
            "O dólar está R$ 5,10, subiu 0,14% hoje."
        );
        assert!(resumo(&cotacoes, Moeda::Todas).contains("O bitcoin R$ 413883"));
    }

    /// Pedir uma moeda que a API não devolveu não pode inventar número nenhum.
    #[test]
    fn moeda_ausente_admite_em_vez_de_chutar() {
        let so_dolar = vec![cotacao("USD", 5.1, 0.0)];
        assert!(resumo(&so_dolar, Moeda::Bitcoin).contains("não consegui"));
    }

    /// A API manda todo número como STRING, e um campo ilegível não pode derrubar as
    /// outras três moedas.
    #[test]
    fn texto_ilegivel_vale_zero_em_vez_de_erro() {
        assert_eq!(numero("5.1001"), 5.1001);
        assert_eq!(numero(""), 0.0);
        assert_eq!(numero("indisponível"), 0.0);
    }

    /// A ida à rede de verdade. `--ignored` porque depende de internet.
    #[tokio::test]
    #[ignore]
    async fn cotacoes_de_verdade() {
        let http = reqwest::Client::new();
        let achadas = cotacoes(&http).await.expect("a AwesomeAPI não respondeu");

        assert_eq!(achadas.len(), 4, "os quatro pares têm que voltar");
        for cotacao in &achadas {
            assert!(cotacao.valor > 0.0, "{} veio zerado", cotacao.codigo);
            println!(
                "{} · {} · {}",
                cotacao.codigo,
                cotacao.formatado(),
                cotacao.movimento()
            );
        }
    }
}
