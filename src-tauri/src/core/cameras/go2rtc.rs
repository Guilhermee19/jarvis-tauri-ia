//! O go2rtc: quem transforma RTSP em coisa que o app consegue mostrar e a IA consegue ler.
//!
//! A webview não decodifica H.264 e o app não embarca decoder — essa é a lacuna inteira.
//! O go2rtc é um binário único que fica no meio: recebe o RTSP das câmeras e entrega
//! **MP4 progressivo** que o `<video>` toca nativamente (`/api/stream.mp4`, o H.264 da
//! câmera repassado sem recodificar) e **JPEG por HTTP** (`/api/frame.jpeg`).
//!
//! O JPEG é o que faz a visão sair de graça. `GET /api/frame.jpeg?src=garagem` devolve um
//! quadro no mesmo formato que a webcam já produz, então [`crate::core::vision`] atende a
//! câmera de segurança sem uma linha de código novo — e vale igual para o DVR Xiongmai e
//! para a V380, que não têm um único protocolo em comum.
//!
//! **`/api/stream.mjpeg` não serve para estas câmeras**, e isso custou um teste para
//! descobrir: ele responde `200` com `Content-Length: 0`, porque produzir MJPEG a partir
//! de H.264 exige transcodificação e o go2rtc só a faz com um ffmpeg configurado. Status
//! de sucesso com corpo vazio é o modo de falha mais caro que existe aqui — não há erro
//! nenhum para investigar, só um quadro que nunca aparece.
//!
//! Ele sobe como serviço em [`crate::core::services`], no mesmo molde do Piper e do
//! Whisper: bate na porta, só sobe se ninguém atender, log em arquivo, e morre junto com
//! o app. A diferença é que este serviço tem **configuração derivada**: o
//! [`escrever_config`] reescreve o `go2rtc.yaml` a partir do catálogo antes de cada
//! subida, porque a lista de câmeras muda e um YAML editado à mão ficaria velho no
//! primeiro cadastro.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;

use super::Camera;

/// Onde a API do go2rtc atende.
///
/// Vizinha das do Whisper (8642) e do Piper (8645), e **não** a 1984 que ele usa por
/// padrão — a política do projeto é fugir do default de cada serviço, porque a porta
/// padrão é justamente a que colide com outra coisa que o usuário já tinha rodando.
pub const PORTA: u16 = 8646;

/// A porta do WebRTC. Só é usada se o player cair nesse modo; em `127.0.0.1` o MSE
/// resolve sem ela.
const PORTA_WEBRTC: u16 = 8647;

/// O RTSP que o próprio go2rtc republica. Preso ao loopback de propósito: reexpor as
/// câmeras para a rede não é o que este app se propôs a fazer.
const PORTA_RTSP: u16 = 8654;

pub const ARQUIVO_DE_CONFIG: &str = "go2rtc.yaml";

/// Um quadro leva o tempo de a câmera responder e o go2rtc decodificar. Generoso porque
/// a PRIMEIRA leitura de um stream frio inclui conectar e sincronizar — depois disso
/// volta em milissegundos.
const TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, thiserror::Error)]
pub enum Go2rtcError {
    #[error("não consegui falar com o go2rtc: {0}")]
    Rede(String),
    #[error("o go2rtc não entregou imagem da câmera \"{camera}\": {detalhe}")]
    SemQuadro { camera: String, detalhe: String },
    #[error("não consegui escrever o {ARQUIVO_DE_CONFIG}: {0}")]
    Config(String),
}

pub fn url() -> String {
    format!("http://127.0.0.1:{PORTA}")
}

/// Reescreve o `go2rtc.yaml` a partir do catálogo.
///
/// Chamado antes de cada subida, e não uma vez na instalação: a lista de câmeras muda, e
/// um arquivo escrito no primeiro cadastro ficaria velho no segundo — com o sintoma de
/// "a câmera nova não aparece" e nenhum erro em lugar nenhum.
///
/// Câmera oculta continua no arquivo. Ocultar é sobre a grade da tela, não sobre o
/// stream existir: uma câmera escondida ainda responde a "olha a garagem" e ainda pode
/// estar sendo vigiada.
pub fn escrever_config(pasta: &Path, cameras: &[Camera]) -> Result<PathBuf, Go2rtcError> {
    fs::create_dir_all(pasta).map_err(|erro| Go2rtcError::Config(erro.to_string()))?;

    let path = pasta.join(ARQUIVO_DE_CONFIG);
    fs::write(&path, yaml(cameras)).map_err(|erro| Go2rtcError::Config(erro.to_string()))?;

    Ok(path)
}

/// O conteúdo do arquivo. Separado da escrita para poder ser testado sem tocar no disco.
fn yaml(cameras: &[Camera]) -> String {
    let mut saida = String::from(
        "# Gerado pelo Jarvis a cada subida do serviço — o que você editar aqui se perde.\n\
         # A lista de câmeras vem do cameras.json.\n\n",
    );

    saida.push_str(&format!("api:\n  listen: \"127.0.0.1:{PORTA}\"\n"));
    saida.push_str(&format!("rtsp:\n  listen: \"127.0.0.1:{PORTA_RTSP}\"\n"));
    saida.push_str(&format!("webrtc:\n  listen: \":{PORTA_WEBRTC}\"\n"));

    // Sem câmera nenhuma o `streams:` fica de fora: uma chave vazia faz o go2rtc
    // reclamar do YAML em vez de subir limpo esperando o primeiro cadastro.
    if cameras.is_empty() {
        return saida;
    }

    saida.push_str("\nstreams:\n");
    for camera in cameras {
        saida.push_str(&format!(
            "  {}: \"{}\"\n",
            aspas(&camera.id),
            aspas(&camera.rtsp())
        ));
    }

    saida
}

/// Escapa o que quebraria uma string YAML entre aspas duplas.
///
/// A senha já vem percent-encoded do [`super::xiongmai`], então na prática não sobra
/// nada para escapar. Isto existe para a URL que o usuário digitou à mão no cadastro —
/// a única que não passou por lá.
fn aspas(bruto: &str) -> String {
    bruto.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Um quadro da câmera, cru.
pub async fn frame_jpeg(http: &reqwest::Client, id: &str) -> Result<Vec<u8>, Go2rtcError> {
    let resposta = http
        .get(format!("{}/api/frame.jpeg", url()))
        .query(&[("src", id)])
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|erro| Go2rtcError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        // O corpo do erro do go2rtc diz a causa real ("streams: not found",
        // "dial tcp: i/o timeout") — sem ele sobra um 500 que não ajuda ninguém.
        let corpo = resposta.text().await.unwrap_or_default();
        return Err(Go2rtcError::SemQuadro {
            camera: id.to_owned(),
            detalhe: format!("{status}: {}", corpo.chars().take(200).collect::<String>()),
        });
    }

    let bytes = resposta
        .bytes()
        .await
        .map_err(|erro| Go2rtcError::Rede(erro.to_string()))?;

    // Corpo vazio com 200 acontece quando o stream conectou mas ainda não chegou quadro
    // decodificável. É diferente de erro de rede e merece dizer isso — senão vira
    // "câmera offline" numa câmera que está perfeitamente online.
    if bytes.is_empty() {
        return Err(Go2rtcError::SemQuadro {
            camera: id.to_owned(),
            detalhe: "o stream abriu mas nenhum quadro chegou a tempo".to_owned(),
        });
    }

    Ok(bytes.to_vec())
}

/// O mesmo quadro, no `data:` URL que a visão e a `<img>` consomem.
///
/// É o formato que [`crate::core::automation`] já produz para a webcam — usar o mesmo é
/// o que deixa [`crate::core::vision::Imagem::do_data_url`] funcionar sem saber que a
/// imagem veio de uma câmera de rede.
pub async fn frame_data_url(http: &reqwest::Client, id: &str) -> Result<String, Go2rtcError> {
    let bytes = frame_jpeg(http, id).await?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(format!("data:image/jpeg;base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::cameras::TipoDeCamera;

    fn dvr() -> Camera {
        Camera {
            id: "garagem".to_owned(),
            nome: "garagem".to_owned(),
            host: "192.168.18.249".to_owned(),
            tipo: TipoDeCamera::Dvr,
            canal: 1,
            usuario: "admin".to_owned(),
            senha: "1234".to_owned(),
            ..Camera::default()
        }
    }

    fn v380() -> Camera {
        Camera {
            id: "quintal".to_owned(),
            nome: "quintal".to_owned(),
            host: "192.168.18.179".to_owned(),
            tipo: TipoDeCamera::Onvif,
            rtsp_url: "rtsp://192.168.18.179/live/ch00_0".to_owned(),
            ..Camera::default()
        }
    }

    #[test]
    fn escreve_um_stream_por_camera() {
        let saida = yaml(&[dvr(), v380()]);

        assert!(saida.contains("streams:\n"));
        assert!(saida.contains("  garagem: \"rtsp://admin:1234@192.168.18.249:554/"));
        assert!(saida.contains("  quintal: \"rtsp://192.168.18.179/live/ch00_0\""));
    }

    #[test]
    fn a_api_escuta_so_no_loopback() {
        let saida = yaml(&[]);

        assert!(saida.contains(&format!("listen: \"127.0.0.1:{PORTA}\"")));
        // Reexpor as câmeras para a rede não é o que este app se propôs a fazer.
        assert!(saida.contains(&format!("listen: \"127.0.0.1:{PORTA_RTSP}\"")));
    }

    /// Um `streams:` vazio faz o go2rtc reclamar do YAML em vez de subir limpo — e ele
    /// PRECISA subir antes do primeiro cadastro, senão não há como cadastrar nada.
    #[test]
    fn sem_camera_nao_escreve_a_chave_streams() {
        assert!(!yaml(&[]).contains("streams:"));
    }

    /// A câmera oculta some da grade, não do arquivo: ela ainda responde por voz e ainda
    /// pode estar sendo vigiada.
    #[test]
    fn camera_oculta_continua_no_arquivo() {
        let mut escondida = dvr();
        escondida.oculto = true;

        assert!(yaml(&[escondida]).contains("  garagem: "));
    }

    #[test]
    fn escapa_aspas_da_url_digitada_a_mao() {
        let mut estranha = v380();
        estranha.rtsp_url = "rtsp://h/a\"b".to_owned();

        assert!(yaml(&[estranha]).contains(r#""rtsp://h/a\"b""#));
    }
}
