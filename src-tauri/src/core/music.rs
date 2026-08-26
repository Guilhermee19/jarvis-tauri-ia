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

use base64::Engine;
use serde::Deserialize;

use crate::core::system::{self, SystemError};

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum MusicError {
    #[error("não entendi o que você quer ouvir")]
    SemBusca,
    #[error("não achei \"{0}\" no Spotify")]
    NaoAchei(String),
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
    pub faixa: Option<String>,
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

    // O deep link faz o app de desktop pular para a faixa e começar a tocar. É por
    // isso que não precisamos do endpoint de playback da API, que exigiria o fluxo
    // completo de OAuth com consentimento no navegador.
    system::abrir_no_spotify(&format!("spotify:track:{}", faixa.id))?;

    Ok(Tocando {
        faixa: Some(format!("{} — {}", faixa.artista, faixa.nome)),
    })
}

/// Nome da fonte para o log de ações.
pub fn modo(client_id: &str, client_secret: &str) -> &'static str {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        "busca no app"
    } else {
        "faixa exata"
    }
}

struct Faixa {
    id: String,
    nome: String,
    artista: String,
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
    }
    #[derive(Deserialize)]
    struct Artista {
        name: String,
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

    let corpo: Resposta = resposta
        .json()
        .await
        .map_err(|erro| MusicError::Rede(erro.to_string()))?;

    let item = corpo
        .tracks
        .items
        .into_iter()
        .next()
        .ok_or_else(|| MusicError::NaoAchei(busca.to_owned()))?;

    Ok(Faixa {
        id: item.id,
        nome: item.name,
        artista: item
            .artists
            .first()
            .map_or_else(|| "?".to_owned(), |a| a.name.clone()),
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
