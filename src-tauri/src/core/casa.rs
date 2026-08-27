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

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;
use serde::{Deserialize, Serialize};

/// As duas portas em que os aparelhos se anunciam. A 6666 é o protocolo 3.1, em texto
/// puro; a 6667 é 3.3+, cifrada. Escutamos as duas porque uma casa costuma ter aparelhos
/// de épocas diferentes.
const PORTAS: [u16; 2] = [6666, 6667];

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
/// O quadro novo (3.5) usa AES-GCM e não é decodificado aqui — mas é RECONHECIDO, para a
/// tela poder dizer que o aparelho existe em vez de fingir que ele não está lá.
const PREFIXO_3_5: [u8; 4] = [0x00, 0x00, 0x66, 0x99];

/// Bytes de cabeçalho antes do payload, e de rodapé (CRC + sufixo) depois dele.
const CABECALHO: usize = 16;
const RODAPE: usize = 8;

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
    /// `false` quando o quadro veio em texto puro (3.1) e quando nós não sabemos ler o
    /// quadro (3.5).
    pub decifrado: bool,
    /// Se este aparelho tem chance de ser controlado pelo caminho que sabemos falar.
    ///
    /// Vai serializado para a tela poder mostrar o 3.5 como "encontrado, mas ainda sem
    /// suporte" em vez de escondê-lo — um aparelho que some da lista vira meia hora
    /// procurando defeito no Wi-Fi.
    pub suportado: bool,
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

fn abrir_portas() -> Result<Vec<UdpSocket>, CasaError> {
    PORTAS
        .iter()
        .map(|&porta| {
            let socket =
                UdpSocket::bind(("0.0.0.0", porta)).map_err(|erro| CasaError::PortaOcupada {
                    porta,
                    detalhe: erro.to_string(),
                })?;

            // Sem timeout, `recv_from` fica preso para sempre na primeira porta silenciosa
            // e a segunda nunca é lida.
            socket
                .set_read_timeout(Some(FATIA))
                .map_err(|erro| CasaError::Rede(erro.to_string()))?;

            Ok(socket)
        })
        .collect()
}

/// Tira o aparelho de um quadro recebido, ou `None` se o quadro não é um anúncio.
///
/// `origem` é o IP de quem mandou o pacote, usado quando o próprio anúncio não traz o
/// campo `ip` — acontece em parte dos firmwares, e o endereço do remetente é a mesma
/// informação por outro caminho.
fn interpretar(quadro: &[u8], origem: &str) -> Option<Aparelho> {
    if quadro.starts_with(&PREFIXO_3_5) {
        // Não sabemos ler (AES-GCM, formato novo), mas sabemos que ELE ESTÁ AÍ. Devolver
        // um registro incompleto é melhor que omitir: a tela avisa, e você descobre que
        // precisa de outro caminho antes de perder tempo procurando o aparelho sumido.
        return Some(Aparelho {
            id: format!("desconhecido@{origem}"),
            ip: origem.to_owned(),
            versao: "3.5".to_owned(),
            produto: None,
            ativo: true,
            decifrado: false,
            suportado: false,
        });
    }

    let corpo = corpo_do_quadro(quadro)?;

    // Texto puro (3.1) ou cifrado (3.3+): em vez de decidir pela porta em que chegou —
    // que nem sempre bate com o firmware —, olhamos o conteúdo. JSON começa com `{`.
    let (json, decifrado) = if corpo.starts_with(b"{") {
        (corpo.to_vec(), false)
    } else {
        (decifrar(corpo)?, true)
    };

    let anuncio: Anuncio = serde_json::from_slice(&json).ok()?;
    let id = anuncio.gw_id.or(anuncio.dev_id)?;
    let versao = anuncio.version.unwrap_or_else(|| "3.1".to_owned());

    Some(Aparelho {
        id,
        ip: anuncio.ip.unwrap_or_else(|| origem.to_owned()),
        suportado: !versao.starts_with("3.5"),
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
    })
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

    quadro.get(CABECALHO..quadro.len() - RODAPE)
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
        assert!(aparelho.suportado);
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

    /// O 3.5 não é lido, mas TEM que aparecer. Um aparelho que some da lista manda a
    /// pessoa procurar defeito no Wi-Fi em vez de no app.
    #[test]
    fn o_protocolo_novo_aparece_mesmo_sem_ser_decifrado() {
        let mut bytes = PREFIXO_3_5.to_vec();
        bytes.extend_from_slice(&[0; 40]);

        let aparelho = interpretar(&bytes, "192.168.0.31").expect("reconheceu o 3.5");

        assert_eq!(aparelho.versao, "3.5");
        assert_eq!(aparelho.ip, "192.168.0.31");
        assert!(!aparelho.decifrado);
        assert!(!aparelho.suportado, "3.5 ainda não dá para controlar");
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
