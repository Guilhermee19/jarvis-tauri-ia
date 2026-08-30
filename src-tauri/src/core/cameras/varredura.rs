//! Procurar câmeras na rede, para não ter que digitar endereço nenhum.
//!
//! O app do fabricante esconde o IP de propósito — ele usa o P2P da nuvem e parte do
//! princípio de que você nunca vai precisar saber onde a câmera está. Só que o JARVIS
//! fala com ela **na rede local**, e aí o endereço é a primeira coisa que falta. Este
//! módulo é a resposta: varre a faixa e diz o que achou.
//!
//! ## Dois estágios, pela mesma razão do [`super::vigia`]
//!
//! O primeiro é barato e burro: abrir um socket em cada `ip:porta` e ver quem atende.
//! São centenas de tentativas que morrem em timeout, e é por isso que ele roda em
//! threads — em série, uma faixa /24 levaria minutos.
//!
//! O segundo só acontece para quem respondeu, e é ele que **identifica**: o ONVIF conta o
//! modelo e entrega a URL do stream, e o banner do RTSP denuncia o DVR Xiongmai. É a
//! diferença entre "tem algo na porta 554" e "é o seu DVR, com 8 canais".
//!
//! ## Por que a sub-rede é um parâmetro
//!
//! O óbvio seria varrer a rede do próprio computador. Não basta: numa casa com roteador
//! em cascata, o PC fica numa faixa e as câmeras em outra — foi exatamente o caso que
//! motivou isto (PC em `192.168.3.x`, câmeras em `192.168.18.x`, alcançáveis por rota).
//! Por isso [`sugestoes_de_prefixo`] devolve uma LISTA: a faixa local mais a de cada
//! câmera já cadastrada.

use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde::Serialize;

use super::{onvif, Camera, TipoDeCamera};

/// Quanto esperar por um socket que provavelmente não existe.
///
/// A maioria das tentativas é contra um IP vazio, e cada uma paga este tempo inteiro.
/// Generoso o bastante para atravessar um roteador entre sub-redes (que foi o caso real
/// aqui), curto o bastante para a varredura inteira caber em segundos.
const TIMEOUT: Duration = Duration::from_millis(700);

/// Quantas sondagens simultâneas.
///
/// A conta que importa: uma faixa /24 com 3 portas são 762 tentativas. Em série, a 700 ms
/// cada, seriam nove minutos. Divididas por 96, ficam em segundos — e são threads que
/// passam a vida inteira bloqueadas num socket, não queimando CPU.
const SONDAS: usize = 96;

/// As portas que separam uma câmera de um host qualquer.
///
/// A 80 fica de fora de propósito: metade dos aparelhos de uma casa a tem aberta, e ela
/// não distingue uma câmera de uma impressora. Estas três, sim.
const PORTAS: [u16; 3] = [
    // 8899 — ONVIF nas câmeras genéricas (V380 e parentes). A que mais informa: responde
    // o modelo e a URL do stream, muitas vezes sem senha nenhuma.
    8899,
    // 554 — RTSP. Presente em tudo que serve vídeo; é o mínimo denominador comum.
    554,
    // 34567 — DVRIP/Sofia, a porta proprietária do Xiongmai. Achá-la aberta é
    // praticamente uma assinatura do DVR que o XMEye abre.
    34567,
];

/// Uma câmera encontrada na rede, pronta para virar cadastro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Achado {
    pub host: String,
    pub tipo: TipoDeCamera,
    /// O que ela é, em linguagem de gente: "IPCAM (HS-Camera_No1)", "DVR Xiongmai
    /// (H264DVR 1.0)". É por esta linha que você reconhece qual é qual.
    pub descricao: String,
    /// A URL do stream, quando o ONVIF a entregou. Vazia no DVR, que não fala ONVIF —
    /// e nele a URL é derivada do canal e das credenciais.
    pub rtsp_url: String,
    /// `true` quando o aparelho pediu autenticação. É o que a tela usa para avisar que
    /// sem usuário e senha esse cadastro não vai mostrar imagem.
    pub precisa_senha: bool,
    /// Já está no catálogo. Continua aparecendo na lista, marcado: sumir faria parecer
    /// que a varredura não a encontrou.
    pub ja_cadastrada: bool,
}

/// As faixas que vale a pena varrer, sem perguntar nada a ninguém.
///
/// A do próprio computador vem primeiro, e as das câmeras já cadastradas em seguida —
/// são a única pista de que existe outra sub-rede alcançável, e numa casa com roteador em
/// cascata elas são justamente onde as câmeras estão.
pub fn sugestoes_de_prefixo(cadastradas: &[Camera]) -> Vec<String> {
    let mut prefixos: Vec<String> = Vec::new();

    if let Some(local) = prefixo_local() {
        prefixos.push(local);
    }

    for camera in cadastradas {
        if let Some(prefixo) = prefixo_de(&camera.host) {
            if !prefixos.contains(&prefixo) {
                prefixos.push(prefixo);
            }
        }
    }

    prefixos
}

/// Os três primeiros octetos do IP desta máquina.
///
/// O truque do `connect` num UDP: ele não manda pacote nenhum (UDP não tem handshake),
/// só faz o sistema escolher a interface que sairia para a internet — e é dela que se lê
/// o endereço. É como se descobre o IP local sem enumerar interfaces, que no Windows
/// exigiria a API do Win32 ou uma dependência a mais.
fn prefixo_local() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;

    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => prefixo_de(&ip.to_string()),
        // IPv6 não tem faixa /24 para varrer, e câmera doméstica não usa.
        IpAddr::V6(_) => None,
    }
}

/// `"192.168.18.249"` vira `"192.168.18"`.
fn prefixo_de(host: &str) -> Option<String> {
    let ip: std::net::Ipv4Addr = host.trim().parse().ok()?;
    let [a, b, c, _] = ip.octets();

    Some(format!("{a}.{b}.{c}"))
}

/// O segundo estágio: transforma "quem atendeu" em "o que é".
///
/// Recebe o resultado de [`sondar_faixa`] em vez de chamá-lo, e os dois ficam separados
/// porque um **bloqueia** e o outro é `async`. Quem os costura é a fronteira, que tem o
/// `spawn_blocking` — este módulo não conhece o Tauri, por regra do `core/`.
pub async fn identificar_todos(
    http: &reqwest::Client,
    candidatos: Vec<(String, Vec<u16>)>,
    cadastradas: &[Camera],
) -> Vec<Achado> {
    let mut achados = Vec::new();

    for (host, portas) in candidatos {
        let ja_cadastrada = cadastradas.iter().any(|camera| camera.host == host);
        achados.push(identificar(http, host, &portas, ja_cadastrada).await);
    }

    achados
}

/// Quem atendeu em `prefixo.1` até `prefixo.254`, e em quais portas.
///
/// **Bloqueia por alguns segundos** — é uma varredura, não uma consulta. Prefixo inválido
/// devolve vazio em vez de montar 254 endereços que só gastariam timeout.
pub fn sondar_faixa(prefixo: &str) -> Vec<(String, Vec<u16>)> {
    let prefixo = prefixo.trim().trim_end_matches('.');
    if prefixo_de(&format!("{prefixo}.1")).is_none() {
        return Vec::new();
    }

    // Uma tarefa por (ip, porta), e não por ip: as três portas de um host morto custam
    // três timeouts em série se ficarem na mesma tarefa, e é o host morto que domina.
    let alvos: Vec<(String, u16)> = (1u8..=254)
        .flat_map(|ultimo| {
            PORTAS
                .iter()
                .map(move |porta| (format!("{prefixo}.{ultimo}"), *porta))
        })
        .collect();

    let proximo = AtomicUsize::new(0);
    let mut abertas: Vec<(String, u16)> = Vec::new();

    std::thread::scope(|escopo| {
        let mut sondas = Vec::new();

        for _ in 0..SONDAS.min(alvos.len()) {
            let proximo = &proximo;
            let alvos = &alvos;

            sondas.push(escopo.spawn(move || {
                let mut minhas = Vec::new();

                loop {
                    let i = proximo.fetch_add(1, Ordering::Relaxed);
                    let Some((host, porta)) = alvos.get(i) else {
                        break;
                    };

                    if atende(host, *porta) {
                        minhas.push((host.clone(), *porta));
                    }
                }

                minhas
            }));
        }

        for sonda in sondas {
            if let Ok(mut achadas) = sonda.join() {
                abertas.append(&mut achadas);
            }
        }
    });

    // Agrupa por host, mantendo a ordem das portas previsível para os testes e para o
    // olho de quem lê o log.
    let mut por_host: std::collections::BTreeMap<String, Vec<u16>> =
        std::collections::BTreeMap::new();
    for (host, porta) in abertas {
        por_host.entry(host).or_default().push(porta);
    }
    for portas in por_host.values_mut() {
        portas.sort_unstable();
    }

    por_host.into_iter().collect()
}

fn atende(host: &str, porta: u16) -> bool {
    let Ok(enderecos) = format!("{host}:{porta}").parse::<SocketAddr>() else {
        return false;
    };

    TcpStream::connect_timeout(&enderecos, TIMEOUT).is_ok()
}

/// Transforma "atendeu na 8899" em "é uma IPCAM, e o stream está aqui".
async fn identificar(
    http: &reqwest::Client,
    host: String,
    portas: &[u16],
    ja_cadastrada: bool,
) -> Achado {
    // ONVIF primeiro: é o único que responde QUEM é, e ainda entrega a URL pronta.
    if portas.contains(&8899) {
        if let Ok(descricao) = onvif::identificar(http, &host).await {
            let rtsp_url = match onvif::perfis(http, &host).await {
                Ok(perfis) => onvif::stream_uri(http, &host, &perfis[0])
                    .await
                    .unwrap_or_default(),
                Err(_) => String::new(),
            };

            return Achado {
                host,
                tipo: TipoDeCamera::Onvif,
                descricao,
                rtsp_url,
                // O ONVIF respondeu sem credencial. Não é garantia de que o RTSP também
                // aceite, mas é o palpite certo: nestas câmeras os dois andam juntos.
                precisa_senha: false,
                ja_cadastrada,
            };
        }
    }

    // Sem ONVIF, quem fala é o banner do RTSP. O `H264DVR` do Xiongmai é o que separa
    // "um DVR com vários canais" de "uma câmera avulsa" — e a diferença muda o cadastro
    // inteiro, porque só o DVR tem canal.
    let banner = portas.contains(&554).then(|| banner_rtsp(&host)).flatten();
    let e_xiongmai = portas.contains(&34567)
        || banner
            .as_deref()
            .is_some_and(|texto| texto.contains("H264DVR"));

    let descricao = match (&banner, e_xiongmai) {
        (Some(banner), true) => format!("DVR Xiongmai / XMEye ({banner})"),
        (Some(banner), false) => format!("Câmera RTSP ({banner})"),
        (None, true) => "DVR Xiongmai / XMEye".to_owned(),
        (None, false) => "Aparelho com RTSP".to_owned(),
    };

    Achado {
        host,
        tipo: if e_xiongmai {
            TipoDeCamera::Dvr
        } else {
            TipoDeCamera::Onvif
        },
        descricao,
        rtsp_url: String::new(),
        // O DVR sempre pede. E, sem ONVIF respondendo, o mais seguro é assumir que sim:
        // o erro de pedir credencial à toa é uma pergunta a mais; o de não pedir é uma
        // câmera cadastrada que nunca mostra imagem.
        precisa_senha: true,
        ja_cadastrada,
    }
}

/// O `Server:` que o RTSP devolve num `OPTIONS`.
///
/// Feito com socket cru porque RTSP não é HTTP — parece, mas o `reqwest` não fala. São
/// três linhas de texto, e o precedente de socket na mão já existe em
/// [`crate::core::casa`].
fn banner_rtsp(host: &str) -> Option<String> {
    use std::io::{Read, Write};

    let endereco: SocketAddr = format!("{host}:554").parse().ok()?;
    let mut fluxo = TcpStream::connect_timeout(&endereco, TIMEOUT).ok()?;
    fluxo.set_read_timeout(Some(TIMEOUT)).ok()?;
    fluxo.set_write_timeout(Some(TIMEOUT)).ok()?;

    let pedido = format!("OPTIONS rtsp://{host}:554/ RTSP/1.0\r\nCSeq: 1\r\n\r\n");
    fluxo.write_all(pedido.as_bytes()).ok()?;

    let mut resposta = [0u8; 1024];
    let lidos = fluxo.read(&mut resposta).ok()?;
    let texto = String::from_utf8_lossy(&resposta[..lidos]);

    texto
        .lines()
        .find_map(|linha| linha.strip_prefix("Server:"))
        .map(|servidor| servidor.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrai_o_prefixo_de_um_ip() {
        assert_eq!(prefixo_de("192.168.18.249").unwrap(), "192.168.18");
        assert_eq!(prefixo_de(" 10.0.0.5 ").unwrap(), "10.0.0");
    }

    /// Um nome de host ou lixo não pode virar prefixo — a varredura montaria 254
    /// endereços inválidos e gastaria o timeout inteiro em cada um.
    #[test]
    fn o_que_nao_e_ipv4_nao_vira_prefixo() {
        assert!(prefixo_de("camera.local").is_none());
        assert!(prefixo_de("").is_none());
        assert!(prefixo_de("192.168.18").is_none());
    }

    /// O caso que motivou o parâmetro: o PC numa faixa, as câmeras em outra. Varrer só a
    /// local não acharia nada.
    #[test]
    fn sugere_a_faixa_das_cameras_ja_cadastradas() {
        let cadastradas = [Camera {
            host: "192.168.18.249".to_owned(),
            ..Camera::default()
        }];

        assert!(sugestoes_de_prefixo(&cadastradas).contains(&"192.168.18".to_owned()));
    }

    /// Duas câmeras na mesma faixa não podem gerar duas varreduras idênticas.
    #[test]
    fn nao_sugere_a_mesma_faixa_duas_vezes() {
        let cadastradas = [
            Camera {
                host: "192.168.18.249".to_owned(),
                ..Camera::default()
            },
            Camera {
                host: "192.168.18.179".to_owned(),
                ..Camera::default()
            },
        ];

        let sugestoes = sugestoes_de_prefixo(&cadastradas);
        let quantas = sugestoes
            .iter()
            .filter(|prefixo| *prefixo == "192.168.18")
            .count();

        assert_eq!(quantas, 1);
    }

    /// Varre a rede DE VERDADE e imprime o que achou.
    ///
    /// Fora do `cargo test` comum porque depende da rede de quem roda — numa máquina de
    /// CI não há câmera nenhuma, e o teste passaria vazio sem provar nada. É a única
    /// forma de saber se a identificação funciona contra firmware real, que é justamente
    /// onde ela pode errar.
    ///
    /// `cargo test --lib -- --ignored --nocapture varre_a_rede_de_verdade`
    #[test]
    #[ignore]
    fn varre_a_rede_de_verdade() {
        let prefixos = sugestoes_de_prefixo(&[]);
        println!("faixas sugeridas sem cadastro nenhum: {prefixos:?}");

        // A faixa das câmeras desta casa fica noutra sub-rede que a do PC, alcançável
        // por rota — é exatamente o caso que o parâmetro de prefixo existe para cobrir.
        let alvo = std::env::var("JARVIS_SCAN").unwrap_or_else(|_| "192.168.18".to_owned());
        println!("varrendo {alvo}.1-254…");

        let relogio = std::time::Instant::now();
        let candidatos = sondar_faixa(&alvo);
        println!(
            "{} host(s) atenderam em {:.1}s",
            candidatos.len(),
            relogio.elapsed().as_secs_f32()
        );
        for (host, portas) in &candidatos {
            println!("  {host} -> {portas:?}");
        }

        let http = reqwest::Client::new();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                for achado in identificar_todos(&http, candidatos, &[]).await {
                    println!(
                        "  {:<16} {:?}  senha={}  {}\n      rtsp: {}",
                        achado.host,
                        achado.tipo,
                        achado.precisa_senha,
                        achado.descricao,
                        if achado.rtsp_url.is_empty() {
                            "(derivada do cadastro)"
                        } else {
                            &achado.rtsp_url
                        }
                    );
                }
            });
    }

    /// Host sem IP válido no cadastro não pode derrubar a sugestão dos outros.
    #[test]
    fn cadastro_com_host_invalido_e_ignorado() {
        let cadastradas = [
            Camera {
                host: "nao-e-um-ip".to_owned(),
                ..Camera::default()
            },
            Camera {
                host: "10.1.2.3".to_owned(),
                ..Camera::default()
            },
        ];

        assert!(sugestoes_de_prefixo(&cadastradas).contains(&"10.1.2".to_owned()));
    }
}
