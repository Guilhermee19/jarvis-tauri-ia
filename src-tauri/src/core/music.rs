//! Tocar música: "toque Charlie Brown Jr só os loucos sabem no spotify".
//!
//! **Com chave toca, sem chave abre a busca.** Achar a faixa exata exige o ID dela no
//! Spotify, e não existe caminho sem credencial — foi medido:
//!
//! | caminho | chave | resultado |
//! | --- | --- | --- |
//! | `spotify:search:<termo>` | não | abre o app na busca, não toca |
//! | raspar `open.spotify.com` | não | página renderizada por JS, sem IDs no HTML |
//! | token anônimo do web player | não | `403 URL Blocked` |
//! | Deezer `/search` | não | acha a faixa certa, mas devolve ID do Deezer |
//! | song.link (Deezer → Spotify) | não | `401 PUBLIC_API_ACCESS_DEPRECATED` |
//! | **Spotify Web API `/v1/search`** | client_id + secret | **ID exato** |
//!
//! As credenciais são as de *client credentials*: criar um app em
//! developer.spotify.com leva dois minutos, é grátis, e NÃO tem fluxo de OAuth — esse
//! par serve para buscar, e quem toca é o app de desktop que você já usa logado.
//!
//! ponytail: o token de acesso vale 1 hora e não é guardado, então cada música paga
//! ~200 ms a mais para pegar um token novo. Cachear é um `Mutex<Option<(String,
//! Instant)>>` no estado — entra quando incomodar.

use std::time::Duration;

use base64::Engine;
use serde::Deserialize;

use crate::core::system::{self, SystemError};

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum MusicError {
    #[error("não entendi o que você quer ouvir")]
    SemBusca,
    #[error("não achei \"{0}\" no Spotify")]
    NaoAchei(String),
    /// Separado do [`MusicError::NaoAchei`] porque as duas causas são OPOSTAS — uma é
    /// "a busca rodou e voltou vazia", a outra é "a busca nem rodou". Compartilhando a
    /// mensagem, o log mentia e a investigação ia para o lado errado.
    #[error("sem credenciais do Spotify — preencha Client ID e Secret em Configurações")]
    SemCredencial,
    #[error("o Spotify recusou a busca (HTTP {status}) — confira as credenciais em Configurações")]
    Recusado { status: u16 },
    #[error("sem internet para procurar a música: {0}")]
    Rede(String),
    #[error(transparent)]
    Windows(#[from] SystemError),
}

/// O que foi parar na tela. `faixa` é `None` quando não havia credencial e só deu para
/// abrir a busca — a diferença entre "tocando" e "achei, é só dar play".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tocando {
    pub faixa: Option<Faixa>,
}

/// O que o widget precisa mostrar. `capa` e `duracao_ms` vêm de graça na resposta da
/// busca — não custam uma chamada a mais.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Faixa {
    pub id: String,
    pub titulo: String,
    pub artista: String,
    /// URL da arte do álbum. `None` em faixa sem capa cadastrada.
    pub capa: Option<String>,
    pub duracao_ms: u64,
}

impl Faixa {
    pub fn como_texto(&self) -> String {
        format!("{} — {}", self.artista, self.titulo)
    }
}

pub async fn tocar(
    http: &reqwest::Client,
    busca: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<Tocando, MusicError> {
    let busca = busca.trim();
    if busca.is_empty() {
        return Err(MusicError::SemBusca);
    }

    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        // Sem credencial: o melhor possível é deixar a busca aberta no app.
        system::abrir_no_spotify(&format!("spotify:search:{busca}"))?;
        return Ok(Tocando { faixa: None });
    }

    let token = autenticar(http, client_id.trim(), client_secret.trim()).await?;
    let faixa = procurar(http, &token, busca).await?;

    // O deep link faz o app pular PARA a faixa — mas não dá play, o que foi medido.
    // Usar o endpoint de playback da API resolveria, ao custo do fluxo completo de
    // OAuth com consentimento no navegador; a tecla de mídia custa uma linha.
    system::abrir_no_spotify(&format!("spotify:track:{}", faixa.id))?;
    garantir_play().await;

    Ok(Tocando { faixa: Some(faixa) })
}

/// Espera o Spotify ficar pronto e, se ele não começou sozinho, aperta o play.
///
/// A tecla de mídia é um TOGGLE: mandá-la com a música já tocando PAUSA, que é o
/// oposto do pedido. Por isso o título da janela é lido antes — é o sinal de estado
/// mais barato que existe aqui.
///
/// ponytail: se o Spotify JÁ estava tocando outra coisa e o deep link só navegar sem
/// trocar a faixa, o título continua sendo o da música velha, isto vê "tocando" e não
/// faz nada — o usuário ouve a música errada. Não dá para consertar com tecla: play
/// pausaria. A saída seria `PUT /v1/me/player/play` da API, que exige o fluxo completo
/// de OAuth. Só vale construir isso se acontecer de verdade.
async fn garantir_play() {
    // Spotify fechado leva alguns segundos para subir; aberto, responde quase na hora.
    const PASSO: Duration = Duration::from_millis(200);
    const TENTATIVAS: u32 = 40;
    /// Janela pronta e ainda parada por ~1 s: não vai tocar sozinha.
    const PACIENCIA: u32 = 5;

    let mut parado_ha = 0;

    for _ in 0..TENTATIVAS {
        tokio::time::sleep(PASSO).await;

        // `None` = a janela ainda não existe. Continua esperando.
        let Some(titulo) = system::titulo_do_spotify() else {
            continue;
        };

        if !system::esta_parado(&titulo) {
            return; // Começou sozinho — não encostar.
        }

        parado_ha += 1;
        if parado_ha >= PACIENCIA {
            break;
        }
    }

    let _ = system::press(system::MediaKey::PlayPause);
}

/// Metadados de uma faixa a partir de um texto, sem tocar nada.
///
/// Serve para descobrir capa e duração da música que JÁ estava tocando — o título da
/// janela do Spotify dá `"Artista - Música"`, e é isso que vira a consulta. Sem essa
/// ponte, tudo que o Jarvis não iniciou apareceria no widget como um quadrado cinza.
pub async fn buscar(
    http: &reqwest::Client,
    busca: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<Faixa, MusicError> {
    let busca = busca.trim();
    if busca.is_empty() {
        return Err(MusicError::SemBusca);
    }
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        return Err(MusicError::SemCredencial);
    }

    let token = autenticar(http, client_id.trim(), client_secret.trim()).await?;
    procurar(http, &token, busca).await
}

/// Nome da fonte para o log de ações.
pub fn modo(client_id: &str, client_secret: &str) -> &'static str {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        "busca no app"
    } else {
        "faixa exata"
    }
}

/// *Client credentials*: sem usuário, sem redirect, sem consentimento. Só dá acesso a
/// dados públicos — que é exatamente o que a busca precisa.
async fn autenticar(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
) -> Result<String, MusicError> {
    #[derive(Deserialize)]
    struct Token {
        access_token: String,
    }

    let credencial =
        base64::engine::general_purpose::STANDARD.encode(format!("{client_id}:{client_secret}"));

    let resposta = http
        .post("https://accounts.spotify.com/api/token")
        .timeout(TIMEOUT)
        .header("Authorization", format!("Basic {credencial}"))
        // Corpo escrito à mão em vez de `.form()`: aquele método está atrás de uma
        // feature do reqwest, e ligar uma feature inteira por um par chave-valor fixo
        // seria caro por nada.
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await
        .map_err(|erro| MusicError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(MusicError::Recusado {
            status: status.as_u16(),
        });
    }

    let token: Token = resposta
        .json()
        .await
        .map_err(|erro| MusicError::Rede(erro.to_string()))?;

    Ok(token.access_token)
}

async fn procurar(http: &reqwest::Client, token: &str, busca: &str) -> Result<Faixa, MusicError> {
    #[derive(Deserialize)]
    struct Resposta {
        tracks: Faixas,
    }
    #[derive(Deserialize)]
    struct Faixas {
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        id: String,
        name: String,
        artists: Vec<Artista>,
        #[serde(default)]
        duration_ms: u64,
        album: Album,
    }
    #[derive(Deserialize)]
    struct Artista {
        name: String,
    }
    #[derive(Deserialize)]
    struct Album {
        #[serde(default)]
        images: Vec<Imagem>,
    }
    #[derive(Deserialize)]
    struct Imagem {
        url: String,
        #[serde(default)]
        width: u32,
    }

    // `market=BR` importa: sem ele o Spotify devolve faixas indisponíveis por aqui, e
    // o app abre numa tela cinza de "conteúdo indisponível".
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track&limit=1&market=BR",
        urlencode(busca)
    );

    let resposta = http
        .get(&url)
        .timeout(TIMEOUT)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|erro| MusicError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(MusicError::Recusado {
            status: status.as_u16(),
        });
    }

    // Lê como TEXTO antes de desserializar: quando a busca volta vazia, o corpo é a
    // única coisa que diz por quê, e `.json()` o consumiria sem deixar rastro.
    let texto = resposta
        .text()
        .await
        .map_err(|erro| MusicError::Rede(erro.to_string()))?;

    let corpo: Resposta = serde_json::from_str(&texto).map_err(|erro| {
        eprintln!("[jarvis] busca não desserializou ({erro}); corpo: {texto:.400}");
        MusicError::Rede(erro.to_string())
    })?;

    let item = corpo.tracks.items.into_iter().next().ok_or_else(|| {
        eprintln!("[jarvis] busca vazia para {busca:?}; url={url}");
        MusicError::NaoAchei(busca.to_owned())
    })?;

    // A menor capa que sirva: o widget mostra ~64px, e a maior do Spotify é 640×640 —
    // baixar meio megabyte de arte para exibir num quadradinho é desperdício.
    let capa = item
        .album
        .images
        .iter()
        .filter(|imagem| imagem.width >= 160)
        .min_by_key(|imagem| imagem.width)
        .or_else(|| item.album.images.first())
        .map(|imagem| imagem.url.clone());

    Ok(Faixa {
        id: item.id,
        titulo: item.name,
        artista: item
            .artists
            .first()
            .map_or_else(|| "?".to_owned(), |a| a.name.clone()),
        capa,
        duracao_ms: item.duration_ms,
    })
}

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

    /// "Só os Loucos Sabem" tem acento e espaço — concatenar sem encode daria 400.
    #[test]
    fn codifica_a_busca_com_acento() {
        assert_eq!(urlencode("so os loucos"), "so%20os%20loucos");
        assert_eq!(urlencode("só"), "s%C3%B3");
    }

    /// O log de ações mostra isto, e é o que explica ao usuário por que a música
    /// tocou sozinha numa vez e só abriu a busca na outra.
    #[test]
    fn o_modo_depende_das_credenciais() {
        assert_eq!(modo("", ""), "busca no app");
        assert_eq!(modo("id", "   "), "busca no app");
        assert_eq!(modo("id", "segredo"), "faixa exata");
    }
}
