//! A casa inteligente: descobrir os aparelhos que existem na rede.
//!
//! **Positivo e EKAZA são a mesma coisa por baixo: Tuya.** As duas são rebrands da mesma
//! plataforma (como Nova Digital, Tramontina, Elgin e boa parte do que se vende barato
//! aqui), e por isso uma leitura só enxerga as duas — e mais uma dúzia de marcas.
//!
//! **Nada aqui precisa de conta, chave ou internet.** Os aparelhos Tuya anunciam a si
//! mesmos na rede local de tempos em tempos, e este módulo só escuta. É o que permite ter
//! a tela "o que tem na minha casa" antes de qualquer cadastro.
//!
//! O que o anúncio NÃO traz é a `local_key`, que é o segredo para mandar comando. Essa vem
//! da nuvem da Tuya, uma vez, e é o assunto da próxima fase.
//!
//! ## Por que não a crate `tuya-rs`
//!
//! Ela faz isso e mais o protocolo de controle, mas exige `rustc 1.88` e este projeto
//! declara `rust-version = "1.77.2"`. Subir o piso do repo inteiro por uma crate 0.2.1
//! custaria mais que as ~40 linhas de decodificação daqui. Quando a fase de CONTROLE
//! chegar, aí sim vale reavaliar: aquela parte do protocolo é bem menos trivial que esta.

pub mod chaveiro;
pub mod controle;
pub mod nuvem;

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use crate::core::casa::chaveiro::{Chaveiro, Conhecido};

use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
// `KeyInit` acima é a mesma trait que o `aes-gcm` usa para montar a cifra (as duas crates
// vêm do mesmo `crypto-common`), por isso ela não aparece de novo neste `use`.
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use serde::{Deserialize, Serialize};

/// As duas portas em que os aparelhos se anunciam. A 6666 é o protocolo 3.1, em texto
/// puro; a 6667 é 3.3+, cifrada. Escutamos as duas porque uma casa costuma ter aparelhos
/// de épocas diferentes.
const PORTAS: [u16; 2] = [6666, 6667];

/// Onde parte dos aparelhos de 3.5 também fala.
///
/// Esta é **opcional de propósito**: 7000 é porta comum, usada por servidor de
/// desenvolvimento e outras coisas, e não é da Tuya. Se ela não abrir, seguimos com as
/// outras duas — trocar a varredura inteira por uma porta a mais seria um mau negócio.
/// Já 6666 e 6667 ocupadas significam um concorrente de verdade (Home Assistant,
/// tinytuya), e aí vale parar e dizer.
const PORTA_EXTRA: u16 = 7000;

/// A chave do broadcast, que é **pública e a mesma no mundo inteiro**: é o MD5 de
/// `yGAdlopoPVldABfn`, publicado na engenharia reversa do protocolo e usado por todo
/// cliente Tuya que existe (tinytuya, localtuya, tuya-rs).
///
/// Ela não é segredo de ninguém e não dá acesso a nada: serve só para LER o anúncio. Quem
/// protege o controle de verdade é a `local_key`, que é uma por aparelho.
///
/// Fixa em bytes em vez de calculada em tempo de execução para não arrastar uma crate de
/// MD5 para dentro do projeto por causa de dezesseis bytes constantes.
const CHAVE_DO_ANUNCIO: [u8; 16] = [
    0x6c, 0x1e, 0xc8, 0xe2, 0xbb, 0x9b, 0xb5, 0x9a, 0xb5, 0x0b, 0x0d, 0xaf, 0x64, 0x9b, 0x41, 0x0a,
];

/// Quanto tempo a varredura fica escutando.
///
/// Não é um tempo de resposta, é uma **janela de espera**: os aparelhos anunciam sozinhos
/// a cada poucos segundos, e não há como pedir que falem antes da hora. Curto demais e
/// metade da casa não aparece; a experiência é a de um radar, não a de uma busca.
///
/// 10 s e não 5 porque o intervalo de anúncio varia por tipo de aparelho — um hub de
/// infravermelho ou uma tomada em modo de economia falam bem menos que uma lâmpada.
const JANELA: Duration = Duration::from_secs(10);

/// De quanto em quanto tempo alternamos entre as duas portas enquanto esperamos.
const FATIA: Duration = Duration::from_millis(150);

/// Cabeçalho e rodapé do quadro clássico do Tuya (3.1–3.4).
const PREFIXO: [u8; 4] = [0x00, 0x00, 0x55, 0xAA];
/// Cabeçalho do quadro novo (3.5), que troca o AES-ECB pelo AES-GCM.
const PREFIXO_3_5: [u8; 4] = [0x00, 0x00, 0x66, 0x99];

/// Bytes de cabeçalho antes do payload, e de rodapé (CRC + sufixo) depois dele.
const CABECALHO: usize = 16;
const RODAPE: usize = 8;

/// Cabeçalho do 3.5: prefixo, 2 bytes de uso desconhecido, sequência, comando e tamanho.
/// Dois a mais que o do quadro clássico, e é o suficiente para nada mais bater se você
/// tentar reaproveitar as contas de lá.
const CABECALHO_3_5: usize = 18;
/// O trecho selado do 3.5 abre com o nonce do GCM e fecha com a tag.
const NONCE: usize = 12;
const TAG: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum CasaError {
    #[error(
        "não consegui abrir a porta {porta} para escutar a rede. Outro programa de casa inteligente (Home Assistant, tinytuya) pode estar usando ela: {detalhe}"
    )]
    PortaOcupada { porta: u16, detalhe: String },
    #[error("falha ao escutar a rede: {0}")]
    Rede(String),
}

/// Um aparelho anunciado na rede.
///
/// `nome` fica de fora de propósito: o anúncio não traz o nome que você deu no app. Ele
/// vem junto com a `local_key`, da nuvem, na próxima fase — e é o que vai fazer "apaga a
/// luz da cozinha" casar com um id como `bf3a4c9d…`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aparelho {
    pub id: String,
    pub ip: String,
    /// "3.3", "3.4", "3.5"… É **o dado mais importante desta fase**: é ele que decide o
    /// que dá para usar para controlar depois.
    pub versao: String,
    /// O modelo, do jeito que a Tuya identifica. Nem todo aparelho manda.
    pub produto: Option<String>,
    /// Um aparelho já pareado com um app está `true`. `false` costuma ser aparelho novo,
    /// esperando configuração.
    pub ativo: bool,
    /// `false` quando o quadro veio em texto puro (3.1) e quando a decifragem falhou —
    /// caso em que o aparelho aparece só com o endereço.
    pub decifrado: bool,
    /// Se este aparelho tem chance de ser controlado pelo caminho que sabemos falar.
    ///
    /// Vai serializado para a tela poder mostrar o 3.5 como "encontrado, mas ainda sem
    /// suporte" em vez de escondê-lo — um aparelho que some da lista vira meia hora
    /// procurando defeito no Wi-Fi.
    pub suportado: bool,
    /// O nome que você deu no app ("Luz Cozinha"), vindo do chaveiro.
    ///
    /// `None` significa "a nuvem ainda não foi consultada", e é o que faz a tela poder
    /// convidar a importar em vez de mostrar um id de 22 caracteres e deixar por isso
    /// mesmo. O anúncio da rede nunca traz nome nenhum.
    pub nome: Option<String>,
    /// Se temos a `local_key` dele guardada.
    ///
    /// Independente do `suportado`: sem chave não existe comando por mais conhecido que
    /// o protocolo seja, e sobra chave para aparelho que ainda não sabemos comandar.
    /// Juntar os dois num campo só esconderia qual das duas coisas está faltando.
    pub tem_chave: bool,
    /// Se a rede o anunciou **nesta** varredura.
    ///
    /// A lista mistura de propósito quem está aqui agora com quem já esteve: um aparelho
    /// que você desligou da tomada não deve sumir da tela, senão o app parece ter
    /// esquecido dele. Mas os dois não podem parecer a mesma coisa — o que está fora do
    /// ar não obedece a botão nenhum.
    pub presente: bool,
    /// Quando a rede o anunciou pela última vez, em ms. `0` = nunca.
    pub visto_em: i64,
    /// A categoria da Tuya: "dj" (lâmpada), "cz" (tomada), "wg2" (gateway)… Vazia até a
    /// importação acontecer — o anúncio da rede não diz que tipo de coisa ele é.
    ///
    /// A tela usa isto para o ícone; o backend, para saber se faz sentido oferecer um
    /// liga-desliga.
    pub categoria: String,
    /// Se este tipo de aparelho tem um liga-desliga que faça sentido oferecer.
    ///
    /// Independente do `suportado` e do `tem_chave`: um gateway responde a tudo e não
    /// tem o que ligar. Sem esta separação, o botão apareceria nele e alternaria um DP
    /// booleano que ninguém sabe o que faz.
    pub comutavel: bool,
    /// Tirado da lista principal por escolha sua. Continua sendo varrido e continua
    /// obedecendo por voz — ocultar é sobre a tela, não sobre o aparelho.
    pub oculto: bool,
    /// O emissor de infravermelho que emite por este controle, quando ele é um.
    ///
    /// Vazio em aparelho de rede. Preenchido, quer dizer que este cartão é uma TV ou um
    /// ar-condicionado: sem IP, sem protocolo, e comandado por teclas em vez de botão.
    pub emissor: String,
    /// Subaparelho ZigBee: ele não fala na rede, quem fala é o gateway.
    ///
    /// Sem isto a tela o trataria como um aparelho de Wi-Fi que sumiu — "fora do ar",
    /// "visto nunca" — quando ele nunca esteve na rede e nem deveria estar.
    pub subaparelho: bool,
}

/// O JSON que vem dentro do anúncio. Nomes crus da Tuya, e todos opcionais porque cada
/// geração de firmware manda um subconjunto diferente.
#[derive(Deserialize)]
struct Anuncio {
    #[serde(rename = "gwId")]
    gw_id: Option<String>,
    #[serde(rename = "devId")]
    dev_id: Option<String>,
    ip: Option<String>,
    version: Option<String>,
    #[serde(rename = "productKey")]
    product_key: Option<String>,
    active: Option<i64>,
}

/// O resultado de uma varredura.
///
/// `ignorados` existe para separar dois silêncios que parecem iguais na tela e têm causas
/// opostas: **ninguém falou** (rede errada, firewall, aparelho na tomada errada) e
/// **falaram e eu não entendi** (formato que este código não lê). Sem esse número, um
/// aparelho que anuncia num formato desconhecido é indistinguível de um aparelho que não
/// existe — e a pessoa vai procurar defeito no Wi-Fi.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Varredura {
    pub aparelhos: Vec<Aparelho>,
    pub ignorados: usize,
}

/// Escuta a rede por [`JANELA`] e devolve o que se anunciou, sem repetição.
///
/// Bloqueia: quem chama roda em `#[tauri::command(async)]`, que é executado fora da
/// thread principal — mesmo arranjo do `start_recording`.
pub fn descobrir() -> Result<Varredura, CasaError> {
    let sockets = abrir_portas()?;
    let mut achados: BTreeMap<String, Aparelho> = BTreeMap::new();
    let mut ignorados = 0usize;
    let ate = Instant::now() + JANELA;
    let mut buffer = [0u8; 2048];

    while Instant::now() < ate {
        for socket in &sockets {
            // Erro aqui é quase sempre o timeout da fatia — o que significa "ninguém
            // falou ainda", e não uma falha. Um aparelho que não anunciou nesta volta
            // anuncia na próxima.
            let Ok((lidos, origem)) = socket.recv_from(&mut buffer) else {
                continue;
            };

            match interpretar(&buffer[..lidos], &origem.ip().to_string()) {
                // O mesmo aparelho se anuncia várias vezes na janela, e nas duas portas.
                // A chave é o id, então a última leitura simplesmente sobrescreve.
                Some(aparelho) => {
                    achados.insert(aparelho.id.clone(), aparelho);
                }
                None => ignorados += 1,
            }
        }
    }

    Ok(Varredura {
        aparelhos: achados.into_values().collect(),
        ignorados,
    })
}

/// A varredura da rede cruzada com o que o chaveiro já sabe.
///
/// É o que a tela consome: o anúncio diz quem está ligado AGORA e em que IP; o chaveiro
/// diz como cada um se chama e se temos a chave dele. Nenhum dos dois sozinho dá uma
/// lista útil.
///
/// Aparelho que está no chaveiro e não anunciou **não** é inventado aqui. Um item na
/// lista que não responde a comando nenhum seria pior que a ausência dele — e a
/// varredura, agora que lê os três protocolos, não costuma perder ninguém.
pub fn descobrir_com(chaveiro: &Chaveiro) -> Result<Varredura, CasaError> {
    let mut varredura = descobrir()?;

    // Onde cada um estava e que protocolo fala, para o comando por voz não ter que
    // procurar de novo antes de apagar uma luz.
    chaveiro.vistos(
        &varredura
            .aparelhos
            .iter()
            .map(|aparelho| {
                (
                    aparelho.id.as_str(),
                    aparelho.ip.as_str(),
                    aparelho.versao.as_str(),
                )
            })
            .collect::<Vec<_>>(),
    );

    let mut vistos_agora = std::collections::BTreeSet::new();
    for aparelho in &mut varredura.aparelhos {
        vistos_agora.insert(aparelho.id.clone());

        let Some(conhecido) = chaveiro.de(&aparelho.id) else {
            continue;
        };

        // Nome em branco na nuvem acontece (aparelho nunca renomeado no app), e um
        // `Some("")` viraria um cartão sem título na tela.
        let nome = conhecido.nome.trim();
        if !nome.is_empty() {
            aparelho.nome = Some(nome.to_owned());
        }
        aparelho.tem_chave = !conhecido.local_key.trim().is_empty();
        aparelho.visto_em = conhecido.visto_em;
        aparelho.comutavel = controle::tem_liga_desliga(&conhecido.categoria);
        aparelho.categoria = conhecido.categoria;
        aparelho.oculto = conhecido.oculto;
        aparelho.emissor = conhecido.emissor;
        aparelho.subaparelho = !conhecido.cid.is_empty();
    }

    // E quem já foi visto um dia, mas ficou calado desta vez. Entra marcado como ausente
    // em vez de sumir: uma lista que encolhe sozinha faz procurar defeito no Wi-Fi, e o
    // aparelho pode estar simplesmente fora da tomada.
    varredura.aparelhos.extend(
        chaveiro
            .todos()
            .into_iter()
            .filter(|ficha| !vistos_agora.contains(&ficha.id))
            .map(do_chaveiro),
    );

    Ok(varredura)
}

fn abrir_portas() -> Result<Vec<UdpSocket>, CasaError> {
    let mut sockets = PORTAS
        .iter()
        .map(|&porta| {
            let socket =
                UdpSocket::bind(("0.0.0.0", porta)).map_err(|erro| CasaError::PortaOcupada {
                    porta,
                    detalhe: erro.to_string(),
                })?;

            escutar_por_fatias(socket)
        })
        .collect::<Result<Vec<_>, _>>()?;

    if let Ok(socket) = UdpSocket::bind(("0.0.0.0", PORTA_EXTRA)) {
        if let Ok(socket) = escutar_por_fatias(socket) {
            sockets.push(socket);
        }
    }

    Ok(sockets)
}

/// Sem timeout, `recv_from` fica preso para sempre na primeira porta silenciosa e as
/// outras nunca são lidas.
fn escutar_por_fatias(socket: UdpSocket) -> Result<UdpSocket, CasaError> {
    socket
        .set_read_timeout(Some(FATIA))
        .map_err(|erro| CasaError::Rede(erro.to_string()))?;

    Ok(socket)
}

/// Tira o aparelho de um quadro recebido, ou `None` se o quadro não é um anúncio.
///
/// `origem` é o IP de quem mandou o pacote, usado quando o próprio anúncio não traz o
/// campo `ip` — acontece em parte dos firmwares, e o endereço do remetente é a mesma
/// informação por outro caminho.
fn interpretar(quadro: &[u8], origem: &str) -> Option<Aparelho> {
    if quadro.starts_with(&PREFIXO_3_5) {
        // Se o quadro novo não abrir, o aparelho ainda entra na lista com o endereço e
        // mais nada. Omitir seria pior que mostrar pouco: um aparelho que some da tela
        // manda a pessoa procurar defeito no Wi-Fi, e o defeito está aqui.
        return decifrar_gcm(quadro)
            .and_then(|json| montar(&json, origem, "3.5", true))
            .or_else(|| Some(so_o_endereco(origem)));
    }

    let corpo = corpo_do_quadro(quadro)?;

    // Texto puro (3.1) ou cifrado (3.3+): em vez de decidir pela porta em que chegou —
    // que nem sempre bate com o firmware —, olhamos o conteúdo. JSON começa com `{`.
    let (json, decifrado) = if corpo.starts_with(b"{") {
        (corpo.to_vec(), false)
    } else {
        (decifrar(corpo)?, true)
    };

    montar(&json, origem, "3.1", decifrado)
}

/// O aparelho que anunciou num quadro que não conseguimos abrir: o endereço é tudo o que
/// sobra, e ainda assim é o bastante para você saber que ele existe.
fn so_o_endereco(origem: &str) -> Aparelho {
    Aparelho {
        id: format!("desconhecido@{origem}"),
        ip: origem.to_owned(),
        versao: "3.5".to_owned(),
        produto: None,
        ativo: true,
        decifrado: false,
        suportado: false,
        nome: None,
        tem_chave: false,
        presente: true,
        visto_em: 0,
        categoria: String::new(),
        comutavel: false,
        oculto: false,
        emissor: String::new(),
        subaparelho: false,
    }
}

/// Monta o aparelho a partir do JSON do anúncio, já aberto.
///
/// `versao_padrao` é o que vale quando o firmware não manda o campo `version`, e ele sai
/// do FORMATO do quadro, não do conteúdo: chamar um 3.5 de 3.1 marcaria como controlável
/// um aparelho que não é.
fn montar(json: &[u8], origem: &str, versao_padrao: &str, decifrado: bool) -> Option<Aparelho> {
    let anuncio: Anuncio = serde_json::from_slice(json).ok()?;
    let id = anuncio.gw_id.or(anuncio.dev_id)?;
    let versao = anuncio.version.unwrap_or_else(|| versao_padrao.to_owned());

    Some(Aparelho {
        id,
        ip: anuncio.ip.unwrap_or_else(|| origem.to_owned()),
        // Ler o anúncio não é o mesmo que saber mandar comando: o 3.4 e o 3.5 negociam
        // uma chave de sessão antes de aceitar qualquer coisa. Quem sabe a lista é o
        // `controle`, e ele é a única fonte dela.
        suportado: controle::da_para_controlar(&versao),
        versao,
        produto: anuncio.product_key,
        // Sem o campo, assume pareado: é o caso comum, e marcar de menos aqui só
        // assustaria à toa. `match` e não `is_none_or`, que só é estável a partir do
        // Rust 1.82 — o projeto declara 1.77.2 e o clippy daqui reprova o build por isso.
        ativo: match anuncio.active {
            Some(estado) => estado > 0,
            None => true,
        },
        decifrado,
        // A varredura não conhece o chaveiro: ela lê a rede e nada mais. Quem cruza as
        // duas coisas é o `descobrir_com`, e essa separação é o que mantém o parser de
        // quadros testável sem disco nenhum.
        nome: None,
        tem_chave: false,
        presente: true,
        visto_em: 0,
        categoria: String::new(),
        comutavel: false,
        oculto: false,
        emissor: String::new(),
        subaparelho: false,
    })
}

/// A ficha de um aparelho conhecido, no formato que a tela consome.
///
/// `decifrado` sai `true` mesmo sem ter havido anúncio nenhum agora: o campo conta se o
/// que sabemos dele foi lido de verdade, e o que está no chaveiro foi.
fn do_chaveiro(ficha: Conhecido) -> Aparelho {
    Aparelho {
        suportado: controle::da_para_controlar(&ficha.versao),
        tem_chave: !ficha.local_key.trim().is_empty(),
        comutavel: controle::tem_liga_desliga(&ficha.categoria),
        nome: Some(ficha.nome).filter(|nome| !nome.trim().is_empty()),
        produto: Some(ficha.produto).filter(|produto| !produto.is_empty()),
        id: ficha.id,
        ip: ficha.ultimo_ip,
        versao: ficha.versao,
        categoria: ficha.categoria,
        ativo: true,
        decifrado: true,
        presente: false,
        visto_em: ficha.visto_em,
        oculto: ficha.oculto,
        emissor: ficha.emissor,
        subaparelho: !ficha.cid.is_empty(),
    }
}

/// Onde e como falar com um aparelho.
///
/// Existe por causa dos subaparelhos ZigBee: o sensor de porta **não tem endereço,
/// protocolo nem chave** — os três são do gateway, e a única coisa dele é o `cid`.
/// Montar o alvo à mão em cada chamador daria três lugares para esquecer isso.
pub struct Endereco {
    pub id: String,
    pub ip: String,
    pub versao: String,
    pub chave: String,
    pub cid: String,
    pub categoria: String,
}

impl Endereco {
    pub fn alvo(&self) -> controle::Alvo<'_> {
        controle::Alvo {
            id: &self.id,
            ip: &self.ip,
            versao: &self.versao,
            local_key: &self.chave,
            cid: &self.cid,
            categoria: &self.categoria,
        }
    }
}

/// Resolve por onde falar com um aparelho, seguindo o gateway quando for o caso.
///
/// `ip` e `versao` vêm da tela, que tem a varredura mais recente — o backend não guarda o
/// retrato da rede de propósito. Num subaparelho eles são ignorados: quem responde é o
/// gateway, e o endereço dele sai do chaveiro.
pub fn endereco_de(chaveiro: &Chaveiro, id: &str, ip: &str, versao: &str) -> Option<Endereco> {
    let conhecido = chaveiro.de(id)?;

    if conhecido.cid.is_empty() {
        return Some(Endereco {
            id: id.to_owned(),
            ip: ip.to_owned(),
            versao: versao.to_owned(),
            chave: conhecido.local_key,
            cid: String::new(),
            categoria: conhecido.categoria,
        });
    }

    let pai = chaveiro.de(&conhecido.pai)?;

    Some(Endereco {
        id: id.to_owned(),
        ip: pai.ultimo_ip,
        versao: pai.versao,
        chave: pai.local_key,
        cid: conhecido.cid,
        categoria: conhecido.categoria,
    })
}

/// Lê o estado de vários aparelhos de uma vez, agrupando por gateway.
///
/// É o que permite acompanhar sensor: uma porta abre a qualquer momento, e um estado que
/// só é buscado quando alguém clica não é um sensor, é uma consulta.
///
/// O agrupamento não é economia, é correção — aparelho Tuya aceita **uma sessão por
/// vez**, e três conexões ao mesmo gateway competiriam entre si. Aparelho de Wi-Fi cai
/// cada um no seu grupo, porque cada um é o próprio gateway.
pub fn ler_estados(chaveiro: &Chaveiro, ids: &[String]) -> Vec<(String, controle::Detalhe)> {
    let mut por_gateway: BTreeMap<String, Vec<Endereco>> = BTreeMap::new();

    for id in ids {
        let Some(endereco) = endereco_de(chaveiro, id, "", "") else {
            continue;
        };
        // Sem endereço não há a quem perguntar: subaparelho órfão, ou aparelho de rede
        // que a varredura ainda não viu.
        if endereco.ip.trim().is_empty() {
            continue;
        }

        por_gateway
            .entry(endereco.ip.clone())
            .or_default()
            .push(endereco);
    }

    por_gateway
        .into_values()
        .flat_map(|grupo| {
            let alvos: Vec<controle::Alvo<'_>> = grupo.iter().map(Endereco::alvo).collect();

            controle::detalhar_varios(&alvos)
                .into_iter()
                .map(|(id, detalhe)| (id.to_owned(), detalhe))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Tudo o que já se conhece, sem encostar na rede.
///
/// É o que o painel mostra no instante em que abre. Sem isto, toda abertura começava com
/// dez segundos de tela vazia — e um app que esquece o que sabia a cada reinício não
/// parece estar guardando nada.
pub fn conhecidos(chaveiro: &Chaveiro) -> Vec<Aparelho> {
    chaveiro.todos().into_iter().map(do_chaveiro).collect()
}

/// O miolo do quadro clássico: 16 bytes de cabeçalho na frente, 8 de CRC e sufixo atrás.
///
/// O CRC não é conferido de propósito. Ele protege contra corrupção em trânsito, e um
/// quadro corrompido já é rejeitado adiante — ou o AES não abre, ou o JSON não parseia.
/// Conferir traria uma dependência de CRC32 para reprovar exatamente os mesmos pacotes.
fn corpo_do_quadro(quadro: &[u8]) -> Option<&[u8]> {
    if !quadro.starts_with(&PREFIXO) || quadro.len() <= CABECALHO + RODAPE {
        return None;
    }

    let corpo = quadro.get(CABECALHO..quadro.len() - RODAPE)?;

    Some(sem_codigo_de_retorno(corpo))
}

/// Tira os 4 bytes de código de retorno que vêm entre o cabeçalho e o payload.
///
/// **Parte dos anúncios traz esse campo e parte não** — e passar ele adiante por engano
/// não dá erro visível: o AES simplesmente não fecha o bloco, o JSON não parseia, e o
/// aparelho é contado como "pacote ignorado". Foi assim que dois aparelhos desta casa
/// ficaram invisíveis enquanto o terceiro aparecia.
///
/// Distinguir pelo comando do quadro erraria em firmware novo. Pelo conteúdo não erra, e
/// por dois caminhos que se completam: em texto puro o payload abre com `{`; cifrado, ele
/// é obrigatoriamente múltiplo do bloco de 16 do AES, então qualquer resto que sobre é
/// justamente o campo a mais.
fn sem_codigo_de_retorno(corpo: &[u8]) -> &[u8] {
    if corpo.starts_with(b"{") {
        return corpo;
    }

    let Some(payload) = corpo.get(4..) else {
        return corpo;
    };

    if payload.starts_with(b"{") || corpo.len() % 16 == 4 {
        return payload;
    }

    corpo
}

/// AES-128-ECB com a chave pública do anúncio, e o padding PKCS7 removido no fim.
///
/// ECB bloco a bloco na mão em vez de uma crate de modo: são oito linhas, e o modo ECB
/// não tem estado entre blocos — é justamente por isso que ele é fraco, e por isso que
/// aqui, onde a chave é pública e a mensagem é um anúncio, ele não protege nada mesmo.
fn decifrar(cifrado: &[u8]) -> Option<Vec<u8>> {
    // `% 16` e não `is_multiple_of`, estável só a partir do Rust 1.87.
    if cifrado.is_empty() || cifrado.len() % 16 != 0 {
        return None;
    }

    let cifra = Aes128::new_from_slice(&CHAVE_DO_ANUNCIO).ok()?;
    let mut aberto = cifrado.to_vec();

    for bloco in aberto.chunks_exact_mut(16) {
        cifra.decrypt_block(bloco.into());
    }

    // PKCS7: o último byte diz quantos bytes de enchimento existem. Conferir antes de
    // truncar não é preciosismo — lixo cifrado decifra em lixo, e um byte final de 0xFF
    // faria o `truncate` entrar em pânico e derrubar a varredura inteira.
    let enchimento = *aberto.last()? as usize;
    if enchimento == 0 || enchimento > 16 || enchimento > aberto.len() {
        return None;
    }
    aberto.truncate(aberto.len() - enchimento);

    Some(aberto)
}

/// AES-128-GCM, com a MESMA chave pública do anúncio antigo: o 3.5 trocou o modo, não o
/// segredo. É por isso que ler um aparelho novo continua não custando conta nem nuvem.
///
/// O sufixo (`00 00 99 66`) não é conferido, pela mesma razão que o CRC do quadro clássico
/// não é: a tag do GCM já rejeita qualquer coisa que não seja exatamente este quadro,
/// cifrado com esta chave. Conferir o sufixo reprovaria os mesmos pacotes, mais tarde.
fn decifrar_gcm(quadro: &[u8]) -> Option<Vec<u8>> {
    // Os últimos 4 bytes do cabeçalho dizem o tamanho do trecho selado. Confiar neles em
    // vez de medir o buffer é o que permite ignorar sobra no fim do datagrama.
    let tamanho = u32::from_be_bytes(quadro.get(14..CABECALHO_3_5)?.try_into().ok()?) as usize;
    if tamanho <= NONCE + TAG {
        return None;
    }

    let selado = quadro.get(CABECALHO_3_5..CABECALHO_3_5.checked_add(tamanho)?)?;
    let (nonce, cifrado_com_tag) = selado.split_at(NONCE);

    let cifra = Aes128Gcm::new_from_slice(&CHAVE_DO_ANUNCIO).ok()?;
    let mut aberto = cifra
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: cifrado_com_tag,
                // Os 14 bytes entre o prefixo e o trecho selado entram como dado
                // autenticado: não são cifrados, mas a tag cobre eles. Errar essa fatia
                // por um byte faz TODA decifragem falhar, sem dizer o porquê.
                aad: quadro.get(4..CABECALHO_3_5)?,
            },
        )
        .ok()?;

    // Parte dos quadros traz 4 bytes de código de retorno antes do JSON e parte não.
    // Decidir pelo comando erra em firmware novo; o conteúdo não erra: JSON abre com `{`.
    if !aberto.starts_with(b"{") {
        if aberto.len() <= 4 {
            return None;
        }
        aberto.drain(..4);
    }

    // O anúncio costuma vir com zeros de enchimento no fim, e o parse de JSON tropeça
    // neles.
    while aberto.last() == Some(&0) {
        aberto.pop();
    }

    Some(aberto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncrypt;

    /// Monta um quadro clássico com o payload dado, para os testes não dependerem de ter
    /// uma lâmpada na mesa.
    fn quadro(payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PREFIXO);
        bytes.extend_from_slice(&[0; 12]); // sequência, comando, tamanho
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(&[0; 8]); // CRC + sufixo
        bytes
    }

    fn cifrar(aberto: &[u8]) -> Vec<u8> {
        let cifra = Aes128::new_from_slice(&CHAVE_DO_ANUNCIO).expect("chave de 16 bytes");
        let mut bytes = aberto.to_vec();

        let enchimento = 16 - (bytes.len() % 16);
        bytes.extend(std::iter::repeat(enchimento as u8).take(enchimento));

        for bloco in bytes.chunks_exact_mut(16) {
            cifra.encrypt_block(bloco.into());
        }
        bytes
    }

    #[test]
    fn le_o_anuncio_em_texto_puro_do_protocolo_antigo() {
        let json = br#"{"gwId":"abc123","ip":"192.168.0.50","version":"3.1"}"#;
        let aparelho = interpretar(&quadro(json), "192.168.0.50").expect("interpretou");

        assert_eq!(aparelho.id, "abc123");
        assert_eq!(aparelho.ip, "192.168.0.50");
        assert_eq!(aparelho.versao, "3.1");
        assert!(!aparelho.decifrado);
        // Ler o anúncio dele é fácil — é texto puro. MANDAR comando não: o 3.1 assina o
        // quadro com MD5 e manda o payload em base64, que é outro protocolo do 3.3 para
        // cima. Enquanto isso não existir, o cartão dele aparece sem botão.
        assert!(!aparelho.suportado);
    }

    #[test]
    fn le_o_anuncio_cifrado_do_protocolo_atual() {
        let json = br#"{"gwId":"bf9d21","ip":"192.168.0.77","version":"3.3","productKey":"keyxyz","active":2}"#;
        let aparelho =
            interpretar(&quadro(&cifrar(json)), "192.168.0.77").expect("decifrou e interpretou");

        assert_eq!(aparelho.id, "bf9d21");
        assert_eq!(aparelho.versao, "3.3");
        assert_eq!(aparelho.produto.as_deref(), Some("keyxyz"));
        assert!(aparelho.ativo);
        assert!(aparelho.decifrado);
        assert!(aparelho.suportado);
    }

    /// Firmware que não manda `ip` no corpo é comum, e o endereço de quem enviou o pacote
    /// é a mesma informação — sem isso o aparelho apareceria sem como ser alcançado.
    #[test]
    fn sem_ip_no_corpo_usa_o_endereco_de_quem_enviou() {
        let json = br#"{"gwId":"semip","version":"3.3"}"#;
        let aparelho = interpretar(&quadro(&cifrar(json)), "10.0.0.9").expect("interpretou");

        assert_eq!(aparelho.ip, "10.0.0.9");
    }

    /// Monta um quadro 3.5 de verdade — cabeçalho, nonce, selado e sufixo —, para os
    /// testes não dependerem de ter um aparelho novo na tomada.
    fn quadro_3_5(aberto: &[u8]) -> Vec<u8> {
        let nonce = [0x42u8; NONCE];
        let mut bytes = PREFIXO_3_5.to_vec();
        bytes.extend_from_slice(&[0; 10]); // uso desconhecido, sequência, comando
        bytes.extend_from_slice(&((NONCE + aberto.len() + TAG) as u32).to_be_bytes());

        let cifra = Aes128Gcm::new_from_slice(&CHAVE_DO_ANUNCIO).expect("chave de 16 bytes");
        let selado = cifra
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: aberto,
                    aad: &bytes[4..CABECALHO_3_5],
                },
            )
            .expect("cifrou");

        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&selado);
        bytes.extend_from_slice(&[0x00, 0x00, 0x99, 0x66]); // sufixo, que não conferimos
        bytes
    }

    #[test]
    fn le_o_anuncio_do_protocolo_novo() {
        let json = br#"{"gwId":"bf3a4c9d","ip":"192.168.3.26","version":"3.5","productKey":"novo"}"#;
        let aparelho = interpretar(&quadro_3_5(json), "192.168.3.26").expect("decifrou o 3.5");

        assert_eq!(aparelho.id, "bf3a4c9d");
        assert_eq!(aparelho.versao, "3.5");
        assert_eq!(aparelho.produto.as_deref(), Some("novo"));
        assert!(aparelho.decifrado);
        // Quem mantém a lista de quem dá para comandar é o `controle`, e o 3.5 entrou
        // nela quando a derivação da chave de sessão dele foi corrigida — ela é AES-GCM,
        // e não o AES-ECB do 3.4.
        assert!(aparelho.suportado);
    }

    /// Parte dos firmwares põe um código de retorno na frente do JSON e enche o fim com
    /// zeros. Sem cortar os dois, o parse falha e o aparelho cai no registro incompleto.
    #[test]
    fn descarta_o_codigo_de_retorno_e_o_enchimento_do_fim() {
        let mut miolo = vec![0x00, 0x00, 0x00, 0x00];
        miolo.extend_from_slice(br#"{"gwId":"comretcode","version":"3.5"}"#);
        miolo.extend_from_slice(&[0; 3]);

        let aparelho = interpretar(&quadro_3_5(&miolo), "10.0.0.4").expect("decifrou");

        assert_eq!(aparelho.id, "comretcode");
        assert!(aparelho.decifrado);
    }

    /// Um 3.5 que não abre TEM que aparecer assim mesmo. Um aparelho que some da lista
    /// manda a pessoa procurar defeito no Wi-Fi em vez de no app.
    #[test]
    fn o_protocolo_novo_aparece_mesmo_sem_ser_decifrado() {
        let mut bytes = PREFIXO_3_5.to_vec();
        bytes.extend_from_slice(&[0; 40]);

        let aparelho = interpretar(&bytes, "192.168.0.31").expect("reconheceu o 3.5");

        assert_eq!(aparelho.id, "desconhecido@192.168.0.31");
        assert_eq!(aparelho.versao, "3.5");
        assert_eq!(aparelho.ip, "192.168.0.31");
        assert!(!aparelho.decifrado);
        assert!(!aparelho.suportado);
    }

    /// A tag do GCM é o que segura o quadro inteiro: mexer em um byte do cabeçalho, que
    /// nem sequer é cifrado, tem que derrubar a leitura em vez de devolver lixo.
    #[test]
    fn o_quadro_novo_adulterado_nao_abre() {
        let mut bytes = quadro_3_5(br#"{"gwId":"integro","version":"3.5"}"#);
        bytes[6] ^= 0x01; // um bit da sequência, que entra como dado autenticado

        assert!(decifrar_gcm(&bytes).is_none());
        // E mesmo assim o aparelho não some da tela.
        let aparelho = interpretar(&bytes, "10.0.0.5").expect("ainda aparece");
        assert_eq!(aparelho.id, "desconhecido@10.0.0.5");
    }

    /// Uma varredura de verdade, na sua rede, para quando o que está na mesa não bate com
    /// o que os testes de mesa dizem. Fora do `cargo test` comum de propósito: depende de
    /// ter aparelho ligado e fica [`JANELA`] segundos parada.
    ///
    /// `cargo test --lib -- --ignored --nocapture varredura_real`
    #[test]
    #[ignore]
    fn varredura_real() {
        let varredura = descobrir().expect("abriu as portas");

        println!("ignorados: {}", varredura.ignorados);
        for aparelho in &varredura.aparelhos {
            println!("{aparelho:#?}");
        }
    }

    /// Despeja cru, por 30 s, tudo o que chega nas portas — com o começo de cada quadro
    /// em hexa.
    ///
    /// É a ferramenta de quando um aparelho não aparece e nenhum teste de mesa explica o
    /// porquê: ela mostra o cabeçalho de verdade, e foi assim que se descobriu o código de
    /// retorno que fazia dois aparelhos desta casa serem contados como ruído. Fora do
    /// `cargo test` comum porque depende de ter aparelho ligado.
    ///
    /// `cargo test --lib -- --ignored --nocapture escuta_crua`
    #[test]
    #[ignore]
    fn escuta_crua() {
        let sockets = abrir_portas().expect("abriu");
        println!("portas abertas: {}", sockets.len());
        let ate = Instant::now() + Duration::from_secs(30);
        let mut buffer = [0u8; 2048];
        let mut total = 0;
        while Instant::now() < ate {
            for socket in &sockets {
                let Ok((lidos, origem)) = socket.recv_from(&mut buffer) else {
                    continue;
                };
                total += 1;
                let hex: String = buffer[..lidos.min(24)]
                    .iter()
                    .map(|b| format!("{b:02x} "))
                    .collect();
                println!(
                    "{origem} porta_local={:?} {lidos}B  {hex}",
                    socket.local_addr().map(|a| a.port())
                );
            }
        }
        println!("total de pacotes: {total}");
    }

    /// O quadro que dois aparelhos desta casa mandam de verdade: mesmo formato clássico,
    /// mas com 4 bytes de código de retorno entre o cabeçalho e o payload. Sem cortá-los
    /// o AES não fecha o bloco e o aparelho vira "pacote ignorado" na tela.
    #[test]
    fn le_o_anuncio_com_codigo_de_retorno_na_frente() {
        let json = br#"{"gwId":"comcodigo","ip":"192.168.3.12","version":"3.3"}"#;

        let mut corpo = vec![0x00, 0x00, 0x00, 0x00];
        corpo.extend_from_slice(&cifrar(json));

        let aparelho = interpretar(&quadro(&corpo), "192.168.3.12").expect("interpretou");

        assert_eq!(aparelho.id, "comcodigo");
        assert_eq!(aparelho.versao, "3.3");
        assert!(aparelho.decifrado);
    }

    /// O mesmo campo, mas num anúncio em texto puro: aí não há bloco de AES para acusar
    /// o resto, e quem denuncia é o `{` quatro bytes adiante.
    #[test]
    fn le_o_texto_puro_com_codigo_de_retorno_na_frente() {
        let mut corpo = vec![0x00, 0x00, 0x00, 0x00];
        corpo.extend_from_slice(br#"{"gwId":"puro","version":"3.1"}"#);

        let aparelho = interpretar(&quadro(&corpo), "10.0.0.8").expect("interpretou");

        assert_eq!(aparelho.id, "puro");
        assert!(!aparelho.decifrado);
    }

    /// Ruído na porta 6667 é normal (outros protocolos, mDNS mal-endereçado). Nada disso
    /// pode virar um aparelho fantasma na tela.
    #[test]
    fn ignora_o_que_nao_e_anuncio() {
        assert!(interpretar(b"", "1.2.3.4").is_none());
        assert!(interpretar(b"lixo qualquer", "1.2.3.4").is_none());
        // Prefixo certo, corpo que não abre nem como texto nem como AES.
        assert!(interpretar(&quadro(&[0xFF; 32]), "1.2.3.4").is_none());
        // JSON válido, mas sem id: não dá para controlar nem exibir com sentido.
        assert!(interpretar(&quadro(br#"{"ip":"1.2.3.4"}"#), "1.2.3.4").is_none());
    }

    /// Lixo cifrado decifra em lixo, e o último byte vira um "enchimento" arbitrário.
    /// Truncar por ele sem conferir entraria em pânico — e um pacote perdido na porta
    /// 6667 derrubaria a varredura inteira.
    #[test]
    fn o_padding_invalido_nao_derruba_a_leitura() {
        for tentativa in 0..=u8::MAX {
            // Só não pode entrar em pânico. Devolver `None` ou um `Vec` de lixo é
            // igualmente aceitável — quem descarta é o parse do JSON, adiante.
            let _ = decifrar(&[tentativa; 16]);
            let _ = decifrar(&[tentativa; 32]);
        }

        assert!(decifrar(&[0x00; 15]).is_none(), "tamanho fora do bloco");
        assert!(decifrar(&[]).is_none());
    }
}

#[cfg(test)]
mod testes_de_campo {
    use super::*;

    /// Lê os sensores DESTA casa, agrupando por gateway. Fora do `cargo test` comum
    /// porque depende de ter os aparelhos ligados.
    ///
    /// `cargo test --lib -- --ignored --nocapture le_os_sensores_de_verdade`
    #[test]
    #[ignore]
    fn le_os_sensores_de_verdade() {
        let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
            .join("com.jarvis.app");
        let chaveiro = Chaveiro::new(&dir);

        let ids: Vec<String> = chaveiro
            .todos()
            .into_iter()
            .filter(|ficha| !ficha.cid.is_empty())
            .map(|ficha| ficha.id)
            .collect();

        println!("{} subaparelhos", ids.len());
        for (id, detalhe) in ler_estados(&chaveiro, &ids) {
            println!("{id}: {:?}", detalhe.leituras);
        }
    }
}
