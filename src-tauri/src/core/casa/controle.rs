//! Mandar comando: a parte que precisa da `local_key`.
//!
//! TCP na porta 6668, quadro por quadro, direto com o aparelho — **sem nuvem e sem
//! internet**. A nuvem só apareceu uma vez, no `nuvem`, para entregar a chave; daqui em
//! diante ela não participa, e apagar a luz continua funcionando com o roteador sem
//! link.
//!
//! ## Por que não basta mandar "liga"
//!
//! Um aparelho Tuya não tem um comando "ligar". Ele tem *data points* numerados, e qual
//! deles é o interruptor **muda por modelo**: tomada velha usa o `1`, lâmpada moderna
//! usa o `20`, e há firmware que usa `switch_1`. Chutar um número dá um comando aceito
//! em silêncio que não acende nada.
//!
//! Por isso todo comando aqui começa perguntando o estado: a resposta diz quais DPs
//! existem, e o interruptor é escolhido a partir dela em vez de adivinhado.
//!
//! ## Três dialetos
//!
//! | versão | quadro | integridade | sessão |
//! | --- | --- | --- | --- |
//! | 3.3 | `55AA`, AES-ECB | CRC-32 | não |
//! | 3.4 | `55AA`, AES-ECB | HMAC-SHA256 | **sim** |
//! | 3.5 | `6699`, AES-GCM | a tag do GCM | **sim** |
//!
//! O que separa o 3.3 dos outros dois é a **negociação de chave de sessão**: eles não
//! aceitam comando nenhum antes de um aperto de mão em três passos que produz uma chave
//! temporária. A `local_key` deixa de cifrar as mensagens e passa a ser só o segredo que
//! prova quem é quem — o que é uma melhoria real de segurança, e o motivo de esses dois
//! terem dado mais trabalho.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

/// A porta de comando dos aparelhos Tuya. Fixa no firmware; não é configurável no app.
const PORTA: u16 = 6668;

/// Um aparelho na mesma rede responde em milissegundos. 5 s é folga para Wi-Fi ruim, e
/// curto o bastante para o botão não parecer travado quando ele está fora da tomada.
const TIMEOUT: Duration = Duration::from_secs(5);

const PREFIXO: [u8; 4] = [0x00, 0x00, 0x55, 0xAA];
const SUFIXO: [u8; 4] = [0x00, 0x00, 0xAA, 0x55];
const CABECALHO: usize = 16;
/// CRC de 4 bytes mais o sufixo de 4.
const RODAPE: usize = 8;
/// HMAC de 32 bytes mais o sufixo de 4.
const RODAPE_HMAC: usize = 36;

const PREFIXO_GCM: [u8; 4] = [0x00, 0x00, 0x66, 0x99];
const SUFIXO_GCM: [u8; 4] = [0x00, 0x00, 0x99, 0x66];
const CABECALHO_GCM: usize = 18;
const NONCE: usize = 12;
const TAG: usize = 16;

/// Os 15 bytes de cabeçalho de versão, que vão na frente do payload só nos comandos que
/// MUDAM algo. Numa consulta eles viram um quadro que o aparelho ignora sem reclamar.
const CABECALHO_DA_VERSAO: usize = 15;

/// Mudar o valor de um ou mais DPs (3.3).
const CONTROL: u32 = 0x07;
/// Perguntar o estado atual (3.3).
const DP_QUERY: u32 = 0x0a;
/// Os passos do aperto de mão do 3.4 e do 3.5.
const SESS_KEY_NEG_START: u32 = 0x03;
const SESS_KEY_NEG_FINISH: u32 = 0x05;
/// Mudar DPs, na gramática nova (3.4 e 3.5).
const CONTROL_NEW: u32 = 0x0d;
/// Perguntar o estado, na gramática nova.
const DP_QUERY_NEW: u32 = 0x10;

/// Os *data points* de uma lâmpada Tuya. Numerados e sem nome no protocolo: quem diz o
/// que cada um significa é o catálogo dela, não o aparelho.
mod luz {
    /// Liga e desliga (booleano).
    pub const LIGA: &str = "20";
    /// "white" | "colour" | "scene" | "music".
    pub const MODO: &str = "21";
    /// Brilho, de 10 a 1000. Vale no modo branco.
    pub const BRILHO: &str = "22";
    /// Temperatura do branco, de 0 (quente) a 1000 (frio).
    pub const TEMPERATURA: &str = "23";
    /// A cor, em HSV empacotado num texto de 12 dígitos hexadecimais.
    pub const COR: &str = "24";
}

/// O maior valor de brilho, saturação e temperatura na escala da Tuya.
const CHEIO: u16 = 1000;
/// Abaixo disto a lâmpada apaga em vez de escurecer, então é o piso do brilho.
const BRILHO_MINIMO: u16 = 10;

/// Os DPs que costumam ser o interruptor, na ordem em que valem o palpite.
///
/// A ordem não é estética: `1` é o de tomada e interruptor de parede, `20` é o das
/// lâmpadas do padrão novo, e os nomeados aparecem em firmware que abandonou os números.
/// Fora desta lista, vale o primeiro DP booleano que a consulta trouxer — um aparelho
/// que só tem um liga-desliga não tem como errar.
const INTERRUPTORES: [&str; 4] = ["1", luz::LIGA, "switch_1", "switch_led"];

/// Os protocolos que este módulo sabe falar.
///
/// Fonte única: quem decide se o cartão ganha botão na tela e quem recusa a conexão leem
/// daqui. Enquanto forem dois lugares com a mesma lista escrita à mão, um dia a tela
/// oferece um botão que o backend recusa.
///
/// **O 3.5 está fora, e não por falta de código.** O aperto de mão dele funciona — o
/// HMAC confere contra o aparelho de verdade —, mas o primeiro comando depois da sessão
/// é recusado com a conexão fechada, e isso não muda com nenhuma das combinações de
/// comando, envelope, chave ou sequência que foram testadas contra o aparelho. Ele volta
/// para cá quando essa peça aparecer; até lá, um botão que sempre falha seria pior que a
/// ausência dele.
const CONTROLAVEIS: [&str; 3] = ["3.3", "3.4", "3.5"];

/// As categorias da Tuya que TÊM um liga-desliga.
///
/// A lista existe por segurança, não por estética. O `ler_interruptor` cai no "primeiro
/// DP booleano" quando não reconhece nenhum dos suspeitos — e num gateway ZigBee, que
/// expõe um booleano no DP 4 sem que ninguém saiba o que ele faz, isso seria alternar
/// algo desconhecido no meio da casa. Aparelho fora desta lista aparece na tela, com
/// ícone, e sem botão.
const COMUTAVEIS: [&str; 14] = [
    "dj",    // lâmpada
    "xdd",   // luminária de teto
    "fwd",   // luz ambiente
    "dc",    // cordão de luz
    "dd",    // fita de LED
    "gyd",   // luz com sensor de presença
    "tgq",   // dimmer
    "tgkg",  // interruptor dimerizável
    "cz",    // tomada
    "pc",    // régua de tomadas
    "kg",    // interruptor
    "tdq",   // disjuntor
    "fs",    // ventilador
    "fsd",   // ventilador de teto
];

/// Se este TIPO de aparelho tem um liga-desliga que faça sentido oferecer.
///
/// Separado do [`da_para_controlar`] de propósito: um é sobre saber CONVERSAR com o
/// aparelho, o outro sobre haver o que dizer. Um gateway 3.4 responde perfeitamente e
/// não tem o que ligar; uma lâmpada 3.5 tem, e ainda não sabemos falar com ela.
pub fn tem_liga_desliga(categoria: &str) -> bool {
    COMUTAVEIS.contains(&categoria.trim())
}

/// Se dá para mandar comando num aparelho desta versão de protocolo.
pub fn da_para_controlar(versao: &str) -> bool {
    CONTROLAVEIS
        .iter()
        .any(|controlavel| versao.starts_with(controlavel))
}

#[derive(Debug, thiserror::Error)]
pub enum ControleError {
    #[error(
        "ainda não sei mandar comando em aparelho do protocolo {versao}. Ele aparece na lista porque existe, não porque dá para controlar"
    )]
    ProtocoloSemSuporte { versao: String },
    #[error(
        "não tenho a chave de controle deste aparelho. Importe da nuvem no painel Casa — e se já importou, importe de novo: parear o aparelho outra vez troca a chave dele"
    )]
    SemChave,
    #[error(
        "não consegui falar com o aparelho em {ip}: {detalhe}. Confira se ele está na tomada e se este PC está na mesma rede"
    )]
    Conexao { ip: String, detalhe: String },
    #[error(
        "o aparelho respondeu, mas não consegui abrir a resposta. Quase sempre é chave velha: parear o aparelho de novo troca a chave, e a de cá ficou para trás — importe da nuvem outra vez"
    )]
    ChaveErrada,
    /// Separado do [`ControleError::ChaveErrada`] porque o aperto de mão falha ANTES de
    /// qualquer comando: dizer "não consegui abrir a resposta" mandaria procurar no
    /// lugar errado quando o que quebrou foi a abertura da sessão.
    #[error(
        "o aparelho recusou o aperto de mão da sessão (no passo: {passo}). Se a chave está atual, pode ser outro programa de casa inteligente conectado nele ao mesmo tempo — aparelho Tuya aceita uma sessão por vez"
    )]
    Sessao { passo: &'static str },
    #[error(
        "o aparelho respondeu sem dizer o que sabe fazer, então não sei qual botão dele é o liga-desliga"
    )]
    SemInterruptor,
}

/// Para quem se manda o comando. Emprestado porque o chamador já tem tudo isto.
pub struct Alvo<'a> {
    pub id: &'a str,
    pub ip: &'a str,
    pub versao: &'a str,
    pub local_key: &'a str,
    /// A categoria da Tuya. Ela não muda a conversa com o aparelho — muda a LEITURA:
    /// é ela que diz se um booleano é um botão ou o estado de uma porta.
    pub categoria: &'a str,
    /// O identificador do subaparelho dentro do gateway, quando é um.
    ///
    /// Vazio em aparelho de Wi-Fi. Preenchido, **o `ip`, a `versao` e a `local_key` são
    /// os do GATEWAY** — o subaparelho não tem nenhum dos três, e a única coisa que é
    /// dele nesta estrutura é o `cid`.
    pub cid: &'a str,
}

/// O que o aparelho respondeu sobre si.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Estado {
    pub ligado: bool,
    /// Qual DP acabou sendo o interruptor. Vai para a tela no modo de log detalhado —
    /// quando um aparelho não obedece, saber se ele foi comandado pelo `1` ou pelo `20`
    /// é a primeira coisa que se quer olhar.
    pub interruptor: String,
}

/// Um liga-desliga do aparelho.
///
/// Plural porque **um aparelho pode ter vários**: a tomada dupla desta casa responde
/// `1` e `2`, e mostrar só o primeiro deixaria metade dela sem botão.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chave {
    pub dp: String,
    pub rotulo: String,
    pub ligado: bool,
}

/// Uma medida que o aparelho reporta e que não se comanda — o estado de uma porta, a
/// bateria de um sensor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leitura {
    pub rotulo: String,
    pub valor: String,
}

/// O retrato completo de um aparelho, para a tela de detalhes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detalhe {
    pub ligado: bool,
    pub interruptor: String,
    /// `Some` só quando os DPs revelam uma lâmpada. **Não é decidido pela categoria**: a
    /// categoria diz o que a nuvem acha que o aparelho é, e os DPs dizem o que ele
    /// realmente aceita — que é o que importa para desenhar um controle.
    pub luz: Option<Luz>,
    /// Todos os liga-desliga, não só o principal.
    pub chaves: Vec<Chave>,
    /// O que o aparelho mede e não se comanda, já em português.
    pub leituras: Vec<Leitura>,
    /// Os data points crus, do jeito que ele respondeu. É o que a tela mostra quando
    /// você abre os detalhes técnicos, e o que permite descobrir um aparelho que faz
    /// algo que este código ainda não modela.
    pub dps: BTreeMap<String, serde_json::Value>,
}

/// O estado de uma lâmpada, já traduzido do catálogo de DPs da Tuya.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Luz {
    /// "white", "colour", "scene" ou "music".
    pub modo: String,
    /// 10 a 1000.
    pub brilho: u16,
    /// 0 (quente) a 1000 (frio).
    pub temperatura: u16,
    /// 0 a 360.
    pub matiz: u16,
    /// 0 a 1000.
    pub saturacao: u16,
    /// Quais ajustes ESTE aparelho aceita. Uma lâmpada só de branco não tem o DP da cor,
    /// e mostrar um seletor que não faz nada é pior que não mostrar.
    pub tem_cor: bool,
    pub tem_brilho: bool,
    pub tem_branco: bool,
}

/// O que mudar numa lâmpada. Campos ausentes ficam como estão.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ajuste {
    pub ligado: Option<bool>,
    pub brilho: Option<u16>,
    pub temperatura: Option<u16>,
    /// Matiz e saturação andam juntos: a Tuya os guarda no mesmo DP, e mandar um sem o
    /// outro exigiria ler o valor atual só para reescrevê-lo.
    pub matiz: Option<u16>,
    pub saturacao: Option<u16>,
}

/// A gramática de quadros de cada geração do protocolo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialeto {
    /// 3.3: quadro `55AA` fechado por CRC-32, sem sessão.
    Direto,
    /// 3.4: quadro `55AA` fechado por HMAC-SHA256, com sessão.
    Sessao,
    /// 3.5: quadro `6699` em AES-GCM, com sessão.
    Gcm,
}

impl Dialeto {
    fn da_versao(versao: &str) -> Option<Self> {
        match versao {
            versao if versao.starts_with("3.5") => Some(Self::Gcm),
            versao if versao.starts_with("3.4") => Some(Self::Sessao),
            versao if versao.starts_with("3.3") => Some(Self::Direto),
            _ => None,
        }
    }

    fn negocia(self) -> bool {
        self != Self::Direto
    }

    fn cabecalho_da_versao(self) -> [u8; CABECALHO_DA_VERSAO] {
        let mut bytes = [0u8; CABECALHO_DA_VERSAO];
        bytes[..3].copy_from_slice(match self {
            Self::Direto => b"3.3",
            Self::Sessao => b"3.4",
            Self::Gcm => b"3.5",
        });

        bytes
    }

    fn consulta(self) -> u32 {
        match self {
            Self::Direto => DP_QUERY,
            _ => DP_QUERY_NEW,
        }
    }

    fn comando(self) -> u32 {
        match self {
            Self::Direto => CONTROL,
            _ => CONTROL_NEW,
        }
    }

    /// Quantos bytes de rodapé o quadro clássico leva. O 3.4 troca o CRC de 4 bytes por
    /// um HMAC de 32, e errar essa conta faz o aparelho esperar bytes que nunca chegam.
    fn rodape(self) -> usize {
        match self {
            Self::Sessao => RODAPE_HMAC,
            _ => RODAPE,
        }
    }
}

/// Pergunta tudo o que o aparelho sabe dizer sobre si.
///
/// **Não exige que ele tenha um liga-desliga.** Um emissor de infravermelho e um sensor
/// respondem uma lista de data points perfeitamente válida e não têm interruptor nenhum —
/// e era justamente para ver o que ELES sabem fazer que esta tela existe. Recusar aqui
/// transformava a única janela para o desconhecido num erro.
pub fn detalhar(alvo: &Alvo) -> Result<Detalhe, ControleError> {
    let mut sessao = Sessao::abrir(alvo)?;
    let dps = sessao.consultar(alvo)?;

    Ok(detalhe_de(dps, alvo.categoria))
}

fn detalhe_de(dps: BTreeMap<String, serde_json::Value>, categoria: &str) -> Detalhe {
    // `ok()` e não `?`: sem interruptor o cartão fica sem botão, e é só isso.
    let estado = ler_interruptor(&dps).ok();

    Detalhe {
        ligado: estado.as_ref().is_some_and(|estado| estado.ligado),
        interruptor: estado.map(|estado| estado.interruptor).unwrap_or_default(),
        luz: ler_luz(&dps),
        chaves: ler_chaves(&dps, categoria),
        leituras: ler_leituras(&dps, categoria),
        dps,
    }
}

/// As categorias que só MEDEM. Nelas um booleano é uma leitura, e não um botão.
///
/// A distinção não é cosmética: no sensor de porta o data point `1` é "está aberta", e
/// tratá-lo como interruptor faria a tela oferecer um botão que manda o sensor abrir a
/// porta — coisa que ele não faz, e que a lista de suspeitos do `ler_interruptor` teria
/// escolhido sem hesitar.
const SENSORES: [&str; 9] = [
    "mcs",   // contato de porta e janela
    "pir",   // presença por infravermelho
    "hps",   // presença por micro-ondas
    "ywbj",  // fumaça
    "rqbj",  // gás
    "sj",    // vazamento de água
    "wsdcg", // temperatura e umidade
    "ldcg",  // luminosidade
    "mcs2",  // contato, geração nova
];

/// O nome de um data point, quando ele é conhecido.
///
/// A tabela é pequena de propósito. Ela cobre o que existe nesta casa e o que é padrão no
/// catálogo da Tuya; o que não estiver aqui aparece como "DP 7", que é feio e honesto —
/// inventar um nome seria pior, porque ninguém saberia que foi inventado.
fn rotulo(categoria: &str, dp: &str) -> String {
    let conhecido = match (categoria, dp) {
        ("mcs" | "mcs2", "1") => Some("Porta"),
        ("hps", "1") => Some("Presença"),
        ("pir", "1") => Some("Movimento"),
        (_, "1") if SENSORES.contains(&categoria) => Some("Detecção"),
        (_, "2") if SENSORES.contains(&categoria) => Some("Bateria"),
        (_, "3") if SENSORES.contains(&categoria) => Some("Bateria"),
        ("dj" | "xdd" | "fwd" | "dc" | "dd" | "gyd", "20") => Some("Luz"),
        (_, "1" | "switch_1") => Some("Chave 1"),
        (_, "2" | "switch_2") => Some("Chave 2"),
        (_, "3" | "switch_3") => Some("Chave 3"),
        (_, "4" | "switch_4") => Some("Chave 4"),
        (_, "9" | "countdown_1") => Some("Temporizador"),
        _ => None,
    };

    conhecido.map_or_else(|| format!("DP {dp}"), ToOwned::to_owned)
}

fn ler_chaves(dps: &BTreeMap<String, serde_json::Value>, categoria: &str) -> Vec<Chave> {
    if SENSORES.contains(&categoria) {
        return Vec::new();
    }

    dps.iter()
        .filter_map(|(dp, valor)| {
            Some(Chave {
                rotulo: rotulo(categoria, dp),
                ligado: valor.as_bool()?,
                dp: dp.clone(),
            })
        })
        .collect()
}

fn ler_leituras(dps: &BTreeMap<String, serde_json::Value>, categoria: &str) -> Vec<Leitura> {
    let e_sensor = SENSORES.contains(&categoria);

    dps.iter()
        .filter(|(_, valor)| e_sensor || !valor.is_boolean())
        .filter_map(|(dp, valor)| {
            let rotulo = rotulo(categoria, dp);

            let texto = match valor {
                // "Aberta"/"Fechada" e não "true"/"false": o booleano de um sensor é uma
                // FRASE, e o significado dela muda com o aparelho.
                serde_json::Value::Bool(ligado) if categoria.starts_with("mcs") => {
                    if *ligado { "Aberta" } else { "Fechada" }.to_owned()
                }
                serde_json::Value::Bool(ligado) if categoria == "pir" || categoria == "hps" => {
                    if *ligado { "Detectado" } else { "Nada" }.to_owned()
                }
                serde_json::Value::Bool(ligado) => {
                    if *ligado { "Sim" } else { "Não" }.to_owned()
                }
                serde_json::Value::Number(numero) if rotulo == "Bateria" => format!("{numero}%"),
                serde_json::Value::Number(numero) => numero.to_string(),
                serde_json::Value::String(texto) if texto.is_empty() => return None,
                serde_json::Value::String(texto) => texto.clone(),
                _ => return None,
            };

            Some(Leitura {
                rotulo,
                valor: texto,
            })
        })
        .collect()
}

/// Aplica um ajuste de lâmpada e devolve o retrato depois dele.
///
/// Numa conexão só: abrir sessão custa um aperto de mão de três quadros, e arrastar dois
/// deles para mexer no brilho apareceria como atraso no arrastar do controle.
pub fn ajustar(alvo: &Alvo, ajuste: &Ajuste) -> Result<Detalhe, ControleError> {
    let mut sessao = Sessao::abrir(alvo)?;
    let dps = sessao.consultar(alvo)?;
    let estado = ler_interruptor(&dps)?;
    let atual = ler_luz(&dps).ok_or(ControleError::SemInterruptor)?;

    let mut mudancas: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    if let Some(ligado) = ajuste.ligado {
        mudancas.insert(estado.interruptor.clone(), ligado.into());
    }

    // Mexer na cor sem trocar o modo não muda nada visível: a lâmpada continua exibindo
    // o branco. O modo vai JUNTO, e é por isso que ele não é um campo do `Ajuste` — ele
    // é consequência do que você mexeu, não uma escolha à parte.
    if let (Some(matiz), Some(saturacao)) = (ajuste.matiz, ajuste.saturacao) {
        let brilho = ajuste.brilho.unwrap_or(atual.brilho).clamp(BRILHO_MINIMO, CHEIO);
        mudancas.insert(
            luz::COR.to_owned(),
            hsv_em_hexa(matiz.min(360), saturacao.min(CHEIO), brilho).into(),
        );
        mudancas.insert(luz::MODO.to_owned(), "colour".into());
    } else if let Some(brilho) = ajuste.brilho {
        mudancas.insert(
            luz::BRILHO.to_owned(),
            brilho.clamp(BRILHO_MINIMO, CHEIO).into(),
        );
    }

    if let Some(temperatura) = ajuste.temperatura {
        mudancas.insert(luz::TEMPERATURA.to_owned(), temperatura.min(CHEIO).into());
        mudancas.insert(luz::MODO.to_owned(), "white".into());
    }

    if !mudancas.is_empty() {
        sessao.aplicar(alvo, mudancas)?;
    }
    // Fecha ANTES de reler. Depois de um comando o aparelho manda o aviso de mudança por
    // conta própria, e uma consulta na mesma conexão lê esse aviso no lugar da resposta
    // — o que chega aqui como "nenhum data point", indistinguível de aparelho mudo.
    drop(sessao);

    // Relê em vez de presumir: a lâmpada arredonda valores e recusa combinações, e
    // devolver o que foi PEDIDO faria o controle mostrar uma coisa e a luz fazer outra.
    detalhar(alvo)
}

/// Manda data points crus, do jeito que o aparelho os entende.
///
/// Existe para o que este módulo ainda não modela: um emissor de infravermelho manda o
/// código da tecla num data point de texto, um termostato manda a temperatura num
/// número. Modelar cada família daria uma função por tipo de aparelho; isto dá o
/// mecanismo, e a tela decide o que faz sentido oferecer.
pub fn enviar_dps(
    alvo: &Alvo,
    dps: BTreeMap<String, serde_json::Value>,
) -> Result<Detalhe, ControleError> {
    let mut sessao = Sessao::abrir(alvo)?;
    sessao.aplicar(alvo, dps)?;
    // Fecha antes de reler: depois de um comando o aparelho manda o aviso de mudança por
    // conta própria, e uma consulta na mesma conexão leria esse aviso no lugar da
    // resposta.
    drop(sessao);

    detalhar(alvo)
}

/// Traduz os DPs de uma lâmpada. `None` quando não há nenhum sinal de que seja uma.
fn ler_luz(dps: &BTreeMap<String, serde_json::Value>) -> Option<Luz> {
    let tem_cor = dps.contains_key(luz::COR);
    let tem_brilho = dps.contains_key(luz::BRILHO);
    let tem_branco = dps.contains_key(luz::TEMPERATURA);

    // Sem nenhum dos três não é lâmpada — é tomada, sensor, o que for.
    if !tem_cor && !tem_brilho && !tem_branco {
        return None;
    }

    let numero = |chave: &str, padrao: u16| {
        dps.get(chave)
            .and_then(serde_json::Value::as_u64)
            .map(|valor| valor.min(u64::from(CHEIO)) as u16)
            .unwrap_or(padrao)
    };

    let (matiz, saturacao, _) = dps
        .get(luz::COR)
        .and_then(serde_json::Value::as_str)
        .and_then(hexa_em_hsv)
        .unwrap_or((0, 0, CHEIO));

    Some(Luz {
        modo: dps
            .get(luz::MODO)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("white")
            .to_owned(),
        brilho: numero(luz::BRILHO, CHEIO),
        temperatura: numero(luz::TEMPERATURA, CHEIO),
        matiz,
        saturacao,
        tem_cor,
        tem_brilho,
        tem_branco,
    })
}

/// A cor da Tuya é um texto de 12 dígitos hexadecimais: matiz, saturação e valor, quatro
/// dígitos cada. Não é o `#RRGGBB` que todo mundo espera, e confundir os dois dá uma cor
/// aceita e completamente diferente da pedida.
fn hsv_em_hexa(matiz: u16, saturacao: u16, valor: u16) -> String {
    format!("{matiz:04x}{saturacao:04x}{valor:04x}")
}

fn hexa_em_hsv(texto: &str) -> Option<(u16, u16, u16)> {
    if texto.len() < 12 {
        return None;
    }

    let pedaco = |inicio: usize| u16::from_str_radix(texto.get(inicio..inicio + 4)?, 16).ok();

    Some((pedaco(0)?, pedaco(4)?, pedaco(8)?))
}

/// Liga ou desliga, e devolve o estado como o aparelho o confirmou.
///
/// Sempre consulta antes: é o que descobre QUAL data point é o interruptor deste modelo.
/// Custa uma ida e volta a mais numa rede local, e evita mandar um comando que o
/// aparelho aceita calado sem fazer nada.
pub fn ligar(alvo: &Alvo, ligado: bool) -> Result<Estado, ControleError> {
    let mut sessao = Sessao::abrir(alvo)?;
    let dps = sessao.consultar(alvo)?;
    let atual = ler_interruptor(&dps)?;

    sessao.comandar(alvo, &atual.interruptor, ligado)?;

    Ok(Estado {
        ligado,
        interruptor: atual.interruptor,
    })
}

/// Escolhe o data point que é o liga-desliga, e lê o valor dele.
fn ler_interruptor(dps: &BTreeMap<String, serde_json::Value>) -> Result<Estado, ControleError> {
    let escolhido = INTERRUPTORES
        .iter()
        .find(|candidato| dps.get(**candidato).is_some_and(|valor| valor.is_boolean()))
        .map(|candidato| (*candidato).to_owned())
        .or_else(|| {
            // Nenhum dos suspeitos: vale o primeiro booleano que houver. Um aparelho com
            // um interruptor só não tem como errar, e um com vários já erraria de
            // qualquer jeito sem saber o que cada um faz.
            dps.iter()
                .find(|(_, valor)| valor.is_boolean())
                .map(|(chave, _)| chave.clone())
        })
        .ok_or(ControleError::SemInterruptor)?;

    Ok(Estado {
        ligado: dps
            .get(&escolhido)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        interruptor: escolhido,
    })
}

/// Uma conexão aberta com um aparelho.
struct Sessao {
    fluxo: TcpStream,
    dialeto: Dialeto,
    /// A chave em uso. É a `local_key` até o aperto de mão terminar, e a chave de sessão
    /// depois dele — nos dois casos, quem cifra E quem assina.
    chave: [u8; 16],
    sequencia: u32,
}

impl Sessao {
    fn abrir(alvo: &Alvo) -> Result<Self, ControleError> {
        let dialeto =
            Dialeto::da_versao(alvo.versao).ok_or_else(|| ControleError::ProtocoloSemSuporte {
                versao: alvo.versao.to_owned(),
            })?;

        // A `local_key` da Tuya são 16 caracteres. Outro tamanho é chave truncada na
        // cópia ou campo vazio — e o erro de AES que viria depois não diria isso.
        let chave: [u8; 16] = alvo
            .local_key
            .trim()
            .as_bytes()
            .try_into()
            .map_err(|_| ControleError::SemChave)?;

        let endereco = (alvo.ip, PORTA)
            .to_socket_addrs()
            .ok()
            .and_then(|mut enderecos| enderecos.next())
            .ok_or_else(|| ControleError::Conexao {
                ip: alvo.ip.to_owned(),
                detalhe: "endereço inválido".to_owned(),
            })?;

        let fluxo = conectar(&endereco).map_err(|erro| ControleError::Conexao {
            ip: alvo.ip.to_owned(),
            detalhe: erro.to_string(),
        })?;

        let mut sessao = Self {
            fluxo,
            dialeto,
            chave,
            sequencia: 0,
        };

        if dialeto.negocia() {
            sessao.negociar()?;
        }

        Ok(sessao)
    }

    /// O aperto de mão em três passos do 3.4 e do 3.5.
    ///
    /// 1. mandamos um nonce nosso;
    /// 2. o aparelho devolve o nonce dele, mais um HMAC do nosso — que é como ele prova
    ///    conhecer a `local_key` sem mandá-la;
    /// 3. devolvemos o HMAC do nonce dele, provando a mesma coisa do nosso lado.
    ///
    /// A chave de sessão nasce dos dois nonces juntos, e é ela que cifra tudo daí em
    /// diante. **A `local_key` nunca trafega** — é o que torna esses dois protocolos
    /// melhores que o 3.3, e é por isso que eles dão mais trabalho.
    fn negociar(&mut self) -> Result<(), ControleError> {
        let nosso: [u8; 16] = *uuid::Uuid::new_v4().as_bytes();

        let resposta = self
            .trocar_quadro(SESS_KEY_NEG_START, &nosso, false)
            .map_err(|_| ControleError::Sessao { passo: "abertura" })?;

        // 16 bytes de nonce dele mais 32 do HMAC do nosso.
        if resposta.len() < 48 {
            return Err(ControleError::Sessao {
                passo: "resposta curta demais",
            });
        }
        let (dele, prova) = resposta.split_at(16);

        if assinar(&self.chave, &nosso) != prova[..32] {
            // Ele não reconhece a chave que temos. É chave velha, não rede.
            return Err(ControleError::ChaveErrada);
        }

        // Só MANDA, sem esperar resposta: o aparelho não confirma este passo, ele
        // simplesmente passa a falar na chave nova. Esperar aqui prende a conexão até o
        // timeout inteiro e faz um aperto de mão bem-sucedido parecer recusa.
        self.enviar(SESS_KEY_NEG_FINISH, &assinar(&self.chave, dele), false)
            .map_err(|_| ControleError::Sessao {
                passo: "confirmação",
            })?;

        // Os dois nonces somados bit a bit: a semente da chave de sessão.
        let mut misturado = [0u8; 16];
        for (destino, (nosso, dele)) in misturado.iter_mut().zip(nosso.iter().zip(dele)) {
            *destino = nosso ^ dele;
        }

        // **E aqui os dois protocolos se separam.** O 3.4 cifra a semente em AES-ECB; o
        // 3.5 cifra em AES-GCM, com o NOSSO nonce servindo de IV, e fica com os 16 bytes
        // de texto cifrado (o GCM devolve texto cifrado mais a tag de 16).
        //
        // Usar a conta do 3.4 no 3.5 dá uma chave perfeitamente válida e completamente
        // errada — e o sintoma não é erro nenhum: o aparelho aceita a sessão, recusa o
        // primeiro comando e fecha a conexão sem dizer por quê.
        self.chave = match self.dialeto {
            Dialeto::Gcm => Aes128Gcm::new(&self.chave.into())
                .encrypt(Nonce::from_slice(&nosso[..NONCE]), misturado.as_slice())
                .ok()
                .and_then(|selado| selado.get(..16)?.try_into().ok())
                .ok_or(ControleError::Sessao {
                    passo: "derivação da chave",
                })?,
            _ => {
                Aes128::new(&self.chave.into()).encrypt_block((&mut misturado).into());
                misturado
            }
        };

        Ok(())
    }

    fn consultar(
        &mut self,
        alvo: &Alvo,
    ) -> Result<BTreeMap<String, serde_json::Value>, ControleError> {
        // Com `cid`, a pergunta muda de destinatário: ela vai para o gateway e ele
        // responde pelo subaparelho. Mandar `devId` junto faz ele responder pelos DPs
        // DELE mesmo — foi o que se viu na sondagem, e é um erro que passa despercebido
        // porque a resposta é válida, só que do aparelho errado.
        let pedido = if alvo.cid.is_empty() {
            serde_json::json!({
                "gwId": alvo.id,
                "devId": alvo.id,
                "uid": alvo.id,
                "t": agora_s().to_string(),
            })
        } else {
            serde_json::json!({ "cid": alvo.cid, "t": agora_s() })
        };

        // A consulta é dos comandos que NÃO levam o cabeçalho de versão, nas três
        // versões. Com ele, o aparelho fecha a conexão sem responder — e isso parece
        // aparelho offline.
        let aberto =
            self.trocar_quadro(self.dialeto.consulta(), pedido.to_string().as_bytes(), false)?;

        Ok(extrair_dps(&aberto))
    }

    /// Manda um conjunto de DPs de uma vez.
    ///
    /// De uma vez e não um por um: cada quadro é uma ida e volta, e a lâmpada pisca
    /// entre eles se o modo e a cor chegarem separados.
    fn aplicar(
        &mut self,
        alvo: &Alvo,
        dps: BTreeMap<String, serde_json::Value>,
    ) -> Result<(), ControleError> {
        let pedido = match (self.dialeto, alvo.cid.is_empty()) {
            (_, false) => serde_json::json!({
                "protocol": 5,
                "t": agora_s(),
                "data": { "cid": alvo.cid, "dps": dps },
            }),
            (Dialeto::Direto, _) => serde_json::json!({
                "devId": alvo.id,
                "uid": alvo.id,
                "t": agora_s().to_string(),
                "dps": dps,
            }),
            _ => serde_json::json!({
                "protocol": 5,
                "t": agora_s(),
                "data": { "dps": dps },
            }),
        };

        self.trocar_quadro(self.dialeto.comando(), pedido.to_string().as_bytes(), true)?;

        Ok(())
    }

    fn comandar(
        &mut self,
        alvo: &Alvo,
        interruptor: &str,
        ligado: bool,
    ) -> Result<(), ControleError> {
        // O 3.4 e o 3.5 embrulham o comando num envelope com `protocol` e `data`, e o
        // `t` deles é NÚMERO — no 3.3 é texto. Mandar a forma do 3.3 para um 3.4 dá um
        // quadro que ele aceita e ignora.
        let pedido = match self.dialeto {
            Dialeto::Direto => serde_json::json!({
                "devId": alvo.id,
                "uid": alvo.id,
                "t": agora_s().to_string(),
                "dps": { interruptor: ligado },
            }),
            _ => serde_json::json!({
                "protocol": 5,
                "t": agora_s(),
                "data": { "dps": { interruptor: ligado } },
            }),
        };

        self.trocar_quadro(self.dialeto.comando(), pedido.to_string().as_bytes(), true)?;

        Ok(())
    }

    /// Manda um quadro e devolve o payload da resposta, já aberto.
    fn trocar_quadro(
        &mut self,
        comando: u32,
        payload: &[u8],
        com_cabecalho_de_versao: bool,
    ) -> Result<Vec<u8>, ControleError> {
        self.enviar(comando, payload, com_cabecalho_de_versao)?;
        self.receber()
    }

    /// Manda um quadro e não espera nada de volta.
    ///
    /// Separado do [`Self::trocar_quadro`] porque **nem todo quadro tem resposta**: o
    /// último passo do aperto de mão é só um aviso, e ficar esperando por ele consome o
    /// timeout inteiro e transforma uma sessão que deu certo numa recusa.
    fn enviar(
        &mut self,
        comando: u32,
        payload: &[u8],
        com_cabecalho_de_versao: bool,
    ) -> Result<(), ControleError> {
        // **O cabeçalho de versão fica em lugares diferentes em cada geração.** No 3.3 ele
        // vai EM CLARO, na frente do trecho cifrado: o aparelho precisa lê-lo para saber
        // como decifrar o resto. No 3.4 e no 3.5 ele entra junto com o payload, dentro da
        // cifra, porque a versão já foi acertada no aperto de mão.
        //
        // Cifrar o cabeçalho do 3.3 junto dá um quadro que o aparelho ACEITA e ignora —
        // sem erro, sem resposta diferente, e sem o relé se mexer.
        let cabecalho = com_cabecalho_de_versao.then(|| self.dialeto.cabecalho_da_versao());
        let (claro, prefixo) = match (self.dialeto, cabecalho) {
            (Dialeto::Direto, Some(cabecalho)) => (payload.to_vec(), Some(cabecalho)),
            (_, Some(cabecalho)) => ([&cabecalho[..], payload].concat(), None),
            (_, None) => (payload.to_vec(), None),
        };

        self.sequencia += 1;
        let quadro = empacotar(
            self.dialeto,
            self.sequencia,
            comando,
            &claro,
            prefixo.as_ref().map(|cabecalho| &cabecalho[..]),
            &self.chave,
        );

        self.fluxo
            .write_all(&quadro)
            .and_then(|()| self.fluxo.flush())
            .map_err(|erro| self.caiu(erro))
    }

    fn receber(&mut self) -> Result<Vec<u8>, ControleError> {
        let mut buffer = [0u8; 8192];
        let lidos = self.fluxo.read(&mut buffer).map_err(|erro| self.caiu(erro))?;

        // Comando aceito costuma vir com payload vazio: o quadro em si é a confirmação.
        // Um `Vec` vazio é resposta legítima, e só a falha de ABRIR é erro.
        desempacotar(self.dialeto, &buffer[..lidos], &self.chave).ok_or_else(|| {
            // O cabeçalho cru no log: "não consegui abrir" tem várias causas com a mesma
            // cara, e o comando e o tamanho do quadro separam elas na hora.
            let hexa: String = buffer[..lidos.min(24)]
                .iter()
                .map(|byte| format!("{byte:02x} "))
                .collect();
            eprintln!("[jarvis] resposta que não abriu ({lidos} B): {hexa}");

            ControleError::ChaveErrada
        })
    }

    fn caiu(&self, erro: std::io::Error) -> ControleError {
        ControleError::Conexao {
            ip: self
                .fluxo
                .peer_addr()
                .map(|endereco| endereco.ip().to_string())
                .unwrap_or_else(|_| "aparelho".to_owned()),
            detalhe: erro.to_string(),
        }
    }
}

/// `connect_timeout` e não `TcpStream::connect`: um IP que não responde deixaria a
/// conexão pendurada no timeout do sistema, que em Windows passa de vinte segundos — e
/// um botão que não volta é pior que um botão que erra rápido.
fn conectar(endereco: &SocketAddr) -> std::io::Result<TcpStream> {
    let fluxo = TcpStream::connect_timeout(endereco, TIMEOUT)?;
    fluxo.set_read_timeout(Some(TIMEOUT))?;
    fluxo.set_write_timeout(Some(TIMEOUT))?;

    Ok(fluxo)
}

/// Monta o quadro completo em volta de um payload em claro.
fn empacotar(
    dialeto: Dialeto,
    sequencia: u32,
    comando: u32,
    claro: &[u8],
    prefixo: Option<&[u8]>,
    chave: &[u8; 16],
) -> Vec<u8> {
    if dialeto == Dialeto::Gcm {
        return empacotar_gcm(sequencia, comando, claro, chave);
    }

    // O prefixo é o cabeçalho de versão do 3.3, que viaja FORA da cifra.
    let mut payload = prefixo.unwrap_or_default().to_vec();
    payload.extend(cifrar(chave, claro));
    let rodape = dialeto.rodape();

    let mut quadro = Vec::with_capacity(CABECALHO + payload.len() + rodape);
    quadro.extend_from_slice(&PREFIXO);
    quadro.extend_from_slice(&sequencia.to_be_bytes());
    quadro.extend_from_slice(&comando.to_be_bytes());
    // O campo de tamanho conta o payload MAIS o rodapé, e não só o payload. Contar de
    // menos aqui faz o aparelho esperar por bytes que nunca chegam e fechar a conexão.
    quadro.extend_from_slice(&((payload.len() + rodape) as u32).to_be_bytes());
    quadro.extend_from_slice(&payload);

    // O selo cobre cabeçalho e payload — tudo o que já está no buffer neste ponto.
    if dialeto == Dialeto::Sessao {
        quadro.extend_from_slice(&assinar(chave, &quadro));
    } else {
        quadro.extend_from_slice(&crc32(&quadro).to_be_bytes());
    }
    quadro.extend_from_slice(&SUFIXO);

    quadro
}

/// O quadro do 3.5: cabeçalho de 18 bytes, e o resto selado pelo AES-GCM.
///
/// Os 14 bytes entre o prefixo e o trecho selado entram como dado autenticado (AAD):
/// não são cifrados, mas a tag cobre eles. É a mesma conta do lado da leitura, lá no
/// `descobrir` — errar essa fatia por um byte faz o aparelho descartar tudo em silêncio.
fn empacotar_gcm(sequencia: u32, comando: u32, claro: &[u8], chave: &[u8; 16]) -> Vec<u8> {
    // 12 dos 16 bytes de um UUID v4: aleatórios de verdade, e sem trazer uma crate de
    // random só para isto — o `uuid` já é dependência do projeto.
    let nonce: [u8; NONCE] = uuid::Uuid::new_v4().as_bytes()[..NONCE]
        .try_into()
        .expect("12 dos 16 bytes do UUID");

    let mut quadro = Vec::with_capacity(CABECALHO_GCM + NONCE + claro.len() + TAG + 4);
    quadro.extend_from_slice(&PREFIXO_GCM);
    quadro.extend_from_slice(&[0, 0]);
    quadro.extend_from_slice(&sequencia.to_be_bytes());
    quadro.extend_from_slice(&comando.to_be_bytes());
    quadro.extend_from_slice(&((NONCE + claro.len() + TAG) as u32).to_be_bytes());

    let selado = Aes128Gcm::new(chave.into())
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: claro,
                aad: &quadro[4..CABECALHO_GCM],
            },
        )
        .unwrap_or_default();

    quadro.extend_from_slice(&nonce);
    quadro.extend_from_slice(&selado);
    quadro.extend_from_slice(&SUFIXO_GCM);

    quadro
}

/// Abre um quadro de resposta e devolve o payload em claro.
fn desempacotar(dialeto: Dialeto, quadro: &[u8], chave: &[u8; 16]) -> Option<Vec<u8>> {
    if dialeto == Dialeto::Gcm {
        return desempacotar_gcm(quadro, chave);
    }

    let rodape = dialeto.rodape();
    if !quadro.starts_with(&PREFIXO) || quadro.len() <= CABECALHO + rodape {
        return None;
    }

    let corpo = sem_codigo_de_retorno(quadro.get(CABECALHO..quadro.len() - rodape)?);
    // A resposta do 3.3 traz o mesmo cabeçalho em claro que a pergunta leva, e ele tem de
    // sair ANTES da decifragem — 15 bytes a mais quebram o bloco do AES.
    let corpo = sem_cabecalho_de_versao(corpo);

    // Texto puro acontece em resposta de erro do próprio aparelho; o resto vem cifrado.
    let aberto = if corpo.starts_with(b"{") || corpo.is_empty() {
        corpo.to_vec()
    } else {
        decifrar(chave, corpo)?
    };

    // E de novo depois: no 3.4 e no 3.5 ele vem por dentro.
    Some(sem_cabecalho_de_versao(&aberto).to_vec())
}

fn desempacotar_gcm(quadro: &[u8], chave: &[u8; 16]) -> Option<Vec<u8>> {
    if !quadro.starts_with(&PREFIXO_GCM) {
        return None;
    }

    let tamanho = u32::from_be_bytes(quadro.get(14..CABECALHO_GCM)?.try_into().ok()?) as usize;
    if tamanho <= NONCE + TAG {
        return None;
    }

    let selado = quadro.get(CABECALHO_GCM..CABECALHO_GCM.checked_add(tamanho)?)?;
    let (nonce, cifrado_com_tag) = selado.split_at(NONCE);

    let aberto = Aes128Gcm::new(chave.into())
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: cifrado_com_tag,
                aad: quadro.get(4..CABECALHO_GCM)?,
            },
        )
        .ok()?;

    // Parte dos quadros traz 4 bytes de código de retorno antes do conteúdo e parte não.
    // Decidir pelo comando erra em firmware novo; o conteúdo não erra.
    let sem_retcode = if aberto.starts_with(b"{") || comeca_com_versao(&aberto) || aberto.len() < 4
    {
        aberto.as_slice()
    } else {
        &aberto[4..]
    };

    Some(sem_cabecalho_de_versao(sem_retcode).to_vec())
}

/// Tira o código de retorno, que é o mais externo dos dois que podem vir na frente.
///
/// Sem tirá-lo, o AES não fecha o bloco e o erro que chega na tela é "chave errada" —
/// mandando trocar uma chave que estava certa. Foi exatamente essa armadilha que fez
/// dois aparelhos sumirem da varredura da rede.
fn sem_codigo_de_retorno(corpo: &[u8]) -> &[u8] {
    if corpo.starts_with(b"{") || comeca_com_versao(corpo) {
        return corpo;
    }

    let Some(resto) = corpo.get(4..) else {
        return corpo;
    };

    // Três denúncias, cada uma para um formato: JSON em texto puro, cabeçalho de versão,
    // e — quando não há nem um nem outro — o resto de bloco que o AES não poderia ter
    // deixado sozinho.
    if resto.starts_with(b"{") || comeca_com_versao(resto) || corpo.len() % 16 == 4 {
        return resto;
    }

    corpo
}

fn sem_cabecalho_de_versao(corpo: &[u8]) -> &[u8] {
    if !comeca_com_versao(corpo) {
        return corpo;
    }

    corpo.get(CABECALHO_DA_VERSAO..).unwrap_or(corpo)
}

fn comeca_com_versao(corpo: &[u8]) -> bool {
    corpo.starts_with(b"3.3") || corpo.starts_with(b"3.4") || corpo.starts_with(b"3.5")
}

/// Os `dps` de uma resposta, venha ela crua ou dentro do envelope do 3.4/3.5.
fn extrair_dps(aberto: &[u8]) -> BTreeMap<String, serde_json::Value> {
    #[derive(serde::Deserialize)]
    struct Resposta {
        #[serde(default)]
        dps: BTreeMap<String, serde_json::Value>,
        /// O 3.4 e o 3.5 respondem `{"protocol":4,"t":…,"data":{"dps":{…}}}` na consulta
        /// e `{"dps":{…}}` no aviso de mudança. Aceitar as duas formas aqui evita um
        /// caminho separado por versão para uma diferença de uma camada de JSON.
        #[serde(default)]
        data: Option<Box<Resposta>>,
    }

    let Ok(resposta) = serde_json::from_slice::<Resposta>(aberto) else {
        return BTreeMap::new();
    };

    if !resposta.dps.is_empty() {
        return resposta.dps;
    }

    resposta.data.map(|dentro| dentro.dps).unwrap_or_default()
}

/// AES-128-ECB com a chave em uso, e o enchimento PKCS7 no fim.
fn cifrar(chave: &[u8; 16], aberto: &[u8]) -> Vec<u8> {
    let cifra = Aes128::new(chave.into());
    let mut bytes = aberto.to_vec();

    let enchimento = 16 - (bytes.len() % 16);
    bytes.extend(std::iter::repeat(enchimento as u8).take(enchimento));

    for bloco in bytes.chunks_exact_mut(16) {
        cifra.encrypt_block(bloco.into());
    }

    bytes
}

fn decifrar(chave: &[u8; 16], cifrado: &[u8]) -> Option<Vec<u8>> {
    // `% 16` e não `is_multiple_of`, estável só a partir do Rust 1.87.
    if cifrado.is_empty() || cifrado.len() % 16 != 0 {
        return None;
    }

    let cifra = Aes128::new(chave.into());
    let mut aberto = cifrado.to_vec();
    for bloco in aberto.chunks_exact_mut(16) {
        cifra.decrypt_block(bloco.into());
    }

    // Chave errada decifra em lixo, e o último byte vira um "enchimento" arbitrário.
    // Truncar sem conferir entraria em pânico e derrubaria o comando inteiro.
    let enchimento = *aberto.last()? as usize;
    if enchimento == 0 || enchimento > 16 || enchimento > aberto.len() {
        return None;
    }
    aberto.truncate(aberto.len() - enchimento);

    Some(aberto)
}

/// HMAC-SHA256: no 3.4 ele substitui o CRC-32, e no aperto de mão é a prova de identidade.
fn assinar(chave: &[u8; 16], mensagem: &[u8]) -> [u8; 32] {
    // Qualificado: o `aes-gcm` e o `hmac` trazem duas traits com um `new_from_slice`
    // cada, e sem dizer qual delas o compilador não escolhe.
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(chave).expect("HMAC aceita 16 bytes");
    mac.update(mensagem);

    mac.finalize().into_bytes().into()
}

/// CRC-32 (IEEE), à mão.
///
/// Bit a bit em vez de tabela: são dez linhas contra 256 constantes, roda uma vez por
/// quadro de algumas centenas de bytes, e evita arrastar uma crate — mesma decisão do
/// `urlencode` do `core::music`.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;

    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            // `wrapping_neg` de 0 ou 1 dá 0x00000000 ou 0xFFFFFFFF: é o `if` do bit
            // baixo escrito sem ramo.
            let mascara = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mascara);
        }
    }

    !crc
}

fn agora_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|desde| desde.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAVE: [u8; 16] = *b"0123456789abcdef";

    /// Vetor conhecido do CRC-32 (IEEE). Se ele estiver errado, o aparelho descarta todo
    /// quadro sem responder nada — e o sintoma é indistinguível de aparelho offline.
    #[test]
    fn o_crc_bate_com_o_vetor_conhecido() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    /// O campo de tamanho conta o payload MAIS o rodapé, e o rodapé do 3.4 é maior que o
    /// do 3.3 — 32 bytes de HMAC contra 4 de CRC.
    #[test]
    fn cada_dialeto_declara_o_proprio_rodape() {
        let curto = empacotar(Dialeto::Direto, 1, CONTROL, b"oi", None, &CHAVE);
        let longo = empacotar(Dialeto::Sessao, 1, CONTROL_NEW, b"oi", None, &CHAVE);

        let tamanho =
            |quadro: &[u8]| u32::from_be_bytes(quadro[12..16].try_into().expect("4 bytes")) as usize;

        assert_eq!(tamanho(&curto), 16 + RODAPE, "um bloco de AES mais CRC e sufixo");
        assert_eq!(tamanho(&longo), 16 + RODAPE_HMAC, "o HMAC são 32 bytes, não 4");
        assert_eq!(longo.len() - curto.len(), RODAPE_HMAC - RODAPE);
    }

    /// O selo cobre cabeçalho e payload, e nada além.
    #[test]
    fn o_selo_cobre_o_cabecalho_e_o_payload() {
        let quadro = empacotar(Dialeto::Direto, 7, DP_QUERY, b"payload", None, &CHAVE);
        let fim = quadro.len() - RODAPE;
        let gravado = u32::from_be_bytes(quadro[fim..fim + 4].try_into().expect("4 bytes"));
        assert_eq!(gravado, crc32(&quadro[..fim]));

        let quadro = empacotar(Dialeto::Sessao, 7, DP_QUERY_NEW, b"payload", None, &CHAVE);
        let fim = quadro.len() - RODAPE_HMAC;
        assert_eq!(quadro[fim..fim + 32], assinar(&CHAVE, &quadro[..fim]));
    }

    /// Ida e volta nos três dialetos: o que sai tem que voltar igual.
    #[test]
    fn o_que_e_empacotado_desempacota() {
        for dialeto in [Dialeto::Direto, Dialeto::Sessao, Dialeto::Gcm] {
            let claro = br#"{"dps":{"1":true}}"#;
            let quadro = empacotar(dialeto, 1, dialeto.consulta(), claro, None, &CHAVE);

            assert_eq!(
                desempacotar(dialeto, &quadro, &CHAVE).expect("abriu"),
                claro,
                "dialeto {dialeto:?}"
            );
        }
    }

    /// O cabeçalho de versão vai só nos comandos que mudam algo, e tem que sair na
    /// leitura — senão ele entra no JSON e o parse morre.
    #[test]
    fn o_cabecalho_de_versao_vai_e_volta() {
        // O 3.3 leva o cabeçalho FORA da cifra; os outros dois, dentro. As duas formas
        // têm que voltar ao mesmo payload na leitura.
        for dialeto in [Dialeto::Direto, Dialeto::Sessao, Dialeto::Gcm] {
            let claro = br#"{"protocol":5,"data":{}}"#;
            let cabecalho = dialeto.cabecalho_da_versao();

            let quadro = if dialeto == Dialeto::Direto {
                empacotar(dialeto, 1, dialeto.comando(), claro, Some(&cabecalho), &CHAVE)
            } else {
                let mut dentro = cabecalho.to_vec();
                dentro.extend_from_slice(claro);
                empacotar(dialeto, 1, dialeto.comando(), &dentro, None, &CHAVE)
            };

            assert_eq!(
                desempacotar(dialeto, &quadro, &CHAVE).expect("abriu"),
                claro,
                "dialeto {dialeto:?}"
            );
        }
    }

    /// A chave de sessão é derivada dos dois nonces, e um bit trocado em qualquer um
    /// deles tem que dar uma chave completamente diferente.
    #[test]
    fn a_chave_de_sessao_depende_dos_dois_nonces() {
        let derivar = |nosso: &[u8; 16], dele: &[u8; 16]| {
            let mut misturado = [0u8; 16];
            for (destino, (a, b)) in misturado.iter_mut().zip(nosso.iter().zip(dele)) {
                *destino = a ^ b;
            }
            Aes128::new(&CHAVE.into()).encrypt_block((&mut misturado).into());
            misturado
        };

        let a = derivar(&[0x11; 16], &[0x22; 16]);
        let b = derivar(&[0x11; 16], &[0x23; 16]);

        assert_ne!(a, b, "trocar o nonce do aparelho tem que trocar a sessão");
        assert_eq!(a, derivar(&[0x11; 16], &[0x22; 16]), "e ser determinística");
    }

    /// O envelope do 3.4/3.5 aninha os `dps` dentro de `data`; o do 3.3 não. As duas
    /// formas têm que sair no mesmo lugar.
    #[test]
    fn le_os_dps_dentro_e_fora_do_envelope() {
        let cru = extrair_dps(br#"{"dps":{"1":true}}"#);
        assert_eq!(cru.get("1").and_then(serde_json::Value::as_bool), Some(true));

        let envelopado = extrair_dps(br#"{"protocol":4,"t":1,"data":{"dps":{"20":false}}}"#);
        assert_eq!(
            envelopado.get("20").and_then(serde_json::Value::as_bool),
            Some(false)
        );

        assert!(extrair_dps(b"nao e json").is_empty());
    }

    /// Chave errada tem que virar `None`, e não pânico: o último byte de lixo decifrado
    /// seria lido como tamanho de enchimento e truncaria além do fim.
    #[test]
    fn chave_errada_nao_derruba_a_leitura() {
        let cifrado = cifrar(&CHAVE, br#"{"dps":{"1":true}}"#);
        let outra = *b"fedcba9876543210";

        let _ = decifrar(&outra, &cifrado);
        for tentativa in 0..=u8::MAX {
            let _ = decifrar(&CHAVE, &[tentativa; 16]);
        }

        // E o GCM, que rejeita pela tag em vez do enchimento.
        let quadro = empacotar(Dialeto::Gcm, 1, DP_QUERY_NEW, b"{}", None, &CHAVE);
        assert!(desempacotar(Dialeto::Gcm, &quadro, &outra).is_none());
    }

    /// `1` vem antes de `20` porque é o de tomada e interruptor de parede, que é o caso
    /// mais comum.
    #[test]
    fn escolhe_o_interruptor_na_ordem_certa() {
        let dps = |pares: &[(&str, serde_json::Value)]| {
            pares
                .iter()
                .map(|(chave, valor)| ((*chave).to_owned(), valor.clone()))
                .collect::<BTreeMap<_, _>>()
        };

        let ambos = dps(&[("1", true.into()), ("20", false.into())]);
        assert_eq!(ler_interruptor(&ambos).expect("achou").interruptor, "1");

        let so_o_novo = dps(&[("20", true.into()), ("21", "white".into())]);
        let escolha = ler_interruptor(&so_o_novo).expect("achou");
        assert_eq!(escolha.interruptor, "20");
        assert!(escolha.ligado);

        let exotico = dps(&[("101", "sei la".into()), ("102", true.into())]);
        assert_eq!(ler_interruptor(&exotico).expect("achou").interruptor, "102");
    }

    /// Aparelho que não expõe nenhum booleano não tem liga-desliga que a gente saiba
    /// achar — e dizer isso é melhor que mandar um comando no escuro.
    #[test]
    fn sem_nenhum_booleano_recusa_em_vez_de_chutar() {
        let so_numeros: BTreeMap<String, serde_json::Value> =
            [("9".to_owned(), 0.into()), ("18".to_owned(), 42.into())]
                .into_iter()
                .collect();

        assert!(matches!(
            ler_interruptor(&so_numeros),
            Err(ControleError::SemInterruptor)
        ));
    }

    #[test]
    fn a_versao_escolhe_o_dialeto() {
        assert_eq!(Dialeto::da_versao("3.3"), Some(Dialeto::Direto));
        assert_eq!(Dialeto::da_versao("3.4"), Some(Dialeto::Sessao));
        assert_eq!(Dialeto::da_versao("3.5"), Some(Dialeto::Gcm));
        assert_eq!(Dialeto::da_versao("3.1"), None, "o 3.1 assina com MD5");
        assert!(!Dialeto::Direto.negocia());
        assert!(Dialeto::Sessao.negocia() && Dialeto::Gcm.negocia());
    }

    /// TEMPORARIO: tenta ler um subaparelho ZigBee pelo gateway.
    #[test]
    #[ignore]
    fn sondar_sub() {
        let ler = |chave: &str| std::env::var(chave).unwrap_or_default();
        let (ip, versao, chave, id, cid) = (
            ler("JARVIS_IP"),
            ler("JARVIS_VERSAO"),
            ler("JARVIS_CHAVE"),
            ler("JARVIS_ID"),
            ler("JARVIS_CID"),
        );
        let alvo = Alvo {
            id: &id,
            ip: &ip,
            versao: &versao,
            local_key: &chave,
            cid: &cid,
            categoria: &ler("JARVIS_CATEGORIA"),
        };

        let variantes = [
            ("so cid", serde_json::json!({ "cid": &cid })),
            (
                "cid + t",
                serde_json::json!({ "cid": &cid, "t": agora_s() }),
            ),
            (
                "envelope",
                serde_json::json!({ "protocol": 4, "t": agora_s(), "data": { "cid": &cid } }),
            ),
            (
                "gwId + cid",
                serde_json::json!({ "gwId": &id, "devId": &id, "cid": &cid, "t": agora_s().to_string() }),
            ),
        ];

        for (nome, corpo) in variantes {
            let Ok(mut sessao) = Sessao::abrir(&alvo) else {
                println!("{nome}: nao abriu a sessao");
                continue;
            };

            match sessao.trocar_quadro(DP_QUERY_NEW, corpo.to_string().as_bytes(), false) {
                Ok(aberto) => println!(
                    "{nome}: {}",
                    String::from_utf8_lossy(&aberto).chars().take(200).collect::<String>()
                ),
                Err(erro) => println!("{nome}: {erro}"),
            }
        }
    }

    /// Conversa de verdade com um aparelho da sua rede, para quando o teste de mesa passa
    /// e o aparelho não obedece. Imprime cada passo do aperto de mão.
    ///
    /// `JARVIS_IP=… JARVIS_VERSAO=3.4 JARVIS_CHAVE=… JARVIS_ID=… \`
    /// `cargo test --lib -- --ignored --nocapture controle_real`
    #[test]
    #[ignore]
    fn controle_real() {
        let ler = |chave: &str| std::env::var(chave).unwrap_or_default();
        let (ip, versao, chave, id) = (
            ler("JARVIS_IP"),
            ler("JARVIS_VERSAO"),
            ler("JARVIS_CHAVE"),
            ler("JARVIS_ID"),
        );

        let alvo = Alvo {
            id: &id,
            ip: &ip,
            versao: &versao,
            local_key: &chave,
            cid: &ler("JARVIS_CID"),
            categoria: &ler("JARVIS_CATEGORIA"),
        };

        match Sessao::abrir(&alvo) {
            Ok(mut sessao) => {
                println!("sessão aberta com {ip} ({versao})");
                match sessao.consultar(&alvo) {
                    Ok(dps) => println!("dps: {dps:#?}"),
                    Err(erro) => println!("consulta falhou: {erro}"),
                }
            }
            Err(erro) => println!("não abriu: {erro}"),
        }

        // Alterna um data point específico, para provar o caminho da tomada dupla.
        let dp = ler("JARVIS_DP");
        if !dp.is_empty() {
            for ligado in [true, false] {
                // Envio e leitura separados por uma pausa: o aparelho confirma o quadro
                // antes de mexer no relé, e reler na mesma hora mostra o estado velho.
                let mut sessao = Sessao::abrir(&alvo).expect("sessao");
                let envio = sessao.aplicar(
                    &alvo,
                    [(dp.clone(), serde_json::Value::Bool(ligado))]
                        .into_iter()
                        .collect(),
                );
                drop(sessao);
                println!("envio {dp}={ligado}: {envio:?}");

                std::thread::sleep(Duration::from_millis(1500));
                match detalhar(&alvo) {
                    Ok(detalhe) => println!("  leu: {:?}", detalhe.dps.get(&dp)),
                    Err(erro) => println!("  leu: {erro}"),
                }
            }
        }

        // Pinta a lâmpada de uma cor e volta ao branco. Prova que o DP da cor e o do
        // modo chegam juntos — mandar a cor sem o modo é aceito e não muda nada visível.
        if ler("JARVIS_COR") == "1" {
            for ajuste in [
                Ajuste { matiz: Some(280), saturacao: Some(1000), brilho: Some(1000), ..Ajuste::default() },
                Ajuste { matiz: Some(30), saturacao: Some(900), ..Ajuste::default() },
                Ajuste { temperatura: Some(1000), brilho: Some(1000), ..Ajuste::default() },
            ] {
                match ajustar(&alvo, &ajuste) {
                    Ok(detalhe) => println!("ajuste -> {:?}", detalhe.luz),
                    Err(erro) => println!("ajuste -> {erro}"),
                }
                std::thread::sleep(Duration::from_millis(1500));
            }
        }

        // Pisca o aparelho e devolve ao estado em que estava. É o único jeito de provar
        // que o comando CHEGOU: um quadro aceito e ignorado tem a mesma cara de sucesso.
        if ler("JARVIS_PISCAR") == "1" {
            for ligado in [false, true] {
                match ligar(&alvo, ligado) {
                    Ok(estado) => println!("{ligado} -> ok, pelo dp {}", estado.interruptor),
                    Err(erro) => println!("{ligado} -> {erro}"),
                }
                std::thread::sleep(Duration::from_millis(1200));
            }
        }
    }
}
