//! ONVIF: perguntar à câmera onde está o stream, e mandar ela virar.
//!
//! É o caminho da V380 e de quase toda câmera IP chinesa genérica. Diferente do DVR
//! Xiongmai, aqui não se adivinha a URL: pergunta-se. O `GetStreamUri` devolve o
//! endereço exato do RTSP, e o `GetProfiles` diz quais perfis existem — normalmente dois,
//! o principal e o secundário.
//!
//! ## SOAP na mão, sem crate de XML
//!
//! As requisições são `format!` e as respostas são lidas por marcador. Não é elegante, e
//! é deliberado: um cliente ONVIF completo é uma dependência grande para usar quatro
//! operações de um subconjunto que estas câmeras implementam pela metade. O parse foi
//! exercitado contra o hardware real antes de virar código.
//!
//! O ponto frágil está coberto: as respostas vêm com os `&` da URL escapados como
//! `&amp;`, e devolver isso cru geraria uma URL que falha sem dizer por quê. Ver
//! [`decodificar`].
//!
//! ## Sem autenticação, e por que isso basta aqui
//!
//! A câmera alvo responde a tudo isto **sem credencial nenhuma** — foi verificado. Não
//! há WS-Security UsernameToken implementado, porque ele pede SHA-1 (que o projeto não
//! tem entre as dependências) para um caso que não existe nesta casa. Uma câmera que
//! exija autenticação vai responder com um SOAP Fault, e é aí que este módulo ganha o
//! header — não antes.

use std::time::Duration;

/// Onde o ONVIF atende nessas câmeras.
///
/// O padrão da especificação é a 80, mas a família da V380 (e boa parte das Xiongmai IP)
/// serve numa porta alta e deixa a 80 para a interface web. Foi a 8899 que respondeu.
const PORTA: u16 = 8899;

/// ONVIF é conversa de rede local: ou responde rápido, ou está desligada.
const TIMEOUT: Duration = Duration::from_secs(6);

/// Quanto tempo a câmera fica girando depois de um comando de voz.
///
/// O `ContinuousMove` do ONVIF gira até mandarem parar — não existe "vire um pouco".
/// Sem este limite, "vira para a esquerda" gira até bater no fim do curso, que não é o
/// que ninguém quer dizer com isso.
const PASSO: Duration = Duration::from_millis(600);

/// Velocidade do giro, de 0 a 1. Meia velocidade porque o passo é curto: mais rápido e
/// o menor comando já atravessa metade da cena.
const VELOCIDADE: f32 = 0.5;

#[derive(Debug, thiserror::Error)]
pub enum OnvifError {
    #[error("não consegui falar com a câmera em {host}: {detalhe}")]
    Rede { host: String, detalhe: String },
    #[error("a câmera recusou o comando: {0}")]
    Recusou(String),
    #[error("a câmera respondeu, mas sem o campo {0} que eu esperava")]
    SemCampo(&'static str),
}

/// Para onde a câmera deve virar.
///
/// Enum fechado, e não uma string: é o que permite `camera_move` ser UM verbo no
/// roteador em vez de quatro. A gramática do modelo só consegue emitir estes valores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direcao {
    Esquerda,
    Direita,
    Cima,
    Baixo,
}

impl Direcao {
    /// O vetor `(pan, tilt)` que o ONVIF espera. `x` positivo é direita, `y` positivo é
    /// cima — a convenção da especificação, que casa com a intuição e não precisa de
    /// inversão.
    fn vetor(self) -> (f32, f32) {
        match self {
            Self::Esquerda => (-VELOCIDADE, 0.0),
            Self::Direita => (VELOCIDADE, 0.0),
            Self::Cima => (0.0, VELOCIDADE),
            Self::Baixo => (0.0, -VELOCIDADE),
        }
    }

    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Esquerda => "esquerda",
            Self::Direita => "direita",
            Self::Cima => "cima",
            Self::Baixo => "baixo",
        }
    }
}

fn endpoint(host: &str) -> String {
    format!("http://{host}:{PORTA}/onvif/device_service")
}

/// Um envelope SOAP com o corpo dado. Os namespaces vão todos declarados na raiz
/// porque estas câmeras não toleram declaração no elemento filho.
fn envelope(corpo: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tds="http://www.onvif.org/ver10/device/wsdl" xmlns:trt="http://www.onvif.org/ver10/media/wsdl" xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl" xmlns:tt="http://www.onvif.org/ver10/schema"><s:Body>{corpo}</s:Body></s:Envelope>"#
    )
}

async fn falar(http: &reqwest::Client, host: &str, corpo: &str) -> Result<String, OnvifError> {
    let resposta = http
        .post(endpoint(host))
        .header("Content-Type", "application/soap+xml; charset=utf-8")
        .timeout(TIMEOUT)
        .body(envelope(corpo))
        .send()
        .await
        .map_err(|erro| OnvifError::Rede {
            host: host.to_owned(),
            detalhe: erro.to_string(),
        })?;

    // O corpo é lido mesmo em erro HTTP: um SOAP Fault chega com status 400 e o motivo
    // legível está justamente nele. Descartar o corpo aqui trocaria "usuário sem
    // permissão de PTZ" por "400 Bad Request".
    let texto = resposta.text().await.map_err(|erro| OnvifError::Rede {
        host: host.to_owned(),
        detalhe: erro.to_string(),
    })?;

    if let Some(motivo) = campo(&texto, "Text") {
        return Err(OnvifError::Recusou(motivo));
    }

    Ok(texto)
}

/// O que a câmera diz que é. Serve para confirmar que o endereço cadastrado é mesmo uma
/// câmera antes de gravar o cadastro.
pub async fn identificar(http: &reqwest::Client, host: &str) -> Result<String, OnvifError> {
    let xml = falar(http, host, "<tds:GetDeviceInformation/>").await?;

    let fabricante = campo(&xml, "Manufacturer").unwrap_or_default();
    let modelo = campo(&xml, "Model").unwrap_or_default();
    let firmware = campo(&xml, "FirmwareVersion").unwrap_or_default();

    Ok(format!("{fabricante} {modelo} ({firmware})")
        .trim()
        .to_owned())
}

/// Os tokens de perfil, na ordem em que a câmera os lista.
///
/// O primeiro costuma ser o principal. São eles que o `GetStreamUri` e o PTZ recebem.
pub async fn perfis(http: &reqwest::Client, host: &str) -> Result<Vec<String>, OnvifError> {
    let xml = falar(http, host, "<trt:GetProfiles/>").await?;
    let tokens = tokens_de_perfil(&xml);

    if tokens.is_empty() {
        return Err(OnvifError::SemCampo("Profiles"));
    }

    Ok(tokens)
}

/// A URL RTSP de um perfil, dita pela própria câmera.
pub async fn stream_uri(
    http: &reqwest::Client,
    host: &str,
    perfil: &str,
) -> Result<String, OnvifError> {
    let corpo = format!(
        "<trt:GetStreamUri><trt:StreamSetup><tt:Stream>RTP-Unicast</tt:Stream>\
         <tt:Transport><tt:Protocol>RTSP</tt:Protocol></tt:Transport></trt:StreamSetup>\
         <trt:ProfileToken>{perfil}</trt:ProfileToken></trt:GetStreamUri>"
    );

    let xml = falar(http, host, &corpo).await?;
    campo(&xml, "Uri").ok_or(OnvifError::SemCampo("Uri"))
}

/// Vira a câmera e para logo depois.
///
/// O par mover/parar é uma coisa só de propósito: o `ContinuousMove` gira até mandarem
/// parar, e deixar o `Stop` a cargo de quem chama é convidar a câmera a girar para
/// sempre quando algo falhar no meio.
pub async fn mover(
    http: &reqwest::Client,
    host: &str,
    perfil: &str,
    direcao: Direcao,
) -> Result<(), OnvifError> {
    let (x, y) = direcao.vetor();
    let corpo = format!(
        "<tptz:ContinuousMove><tptz:ProfileToken>{perfil}</tptz:ProfileToken>\
         <tptz:Velocity><tt:PanTilt x=\"{x}\" y=\"{y}\"/></tptz:Velocity></tptz:ContinuousMove>"
    );

    falar(http, host, &corpo).await?;
    tokio::time::sleep(PASSO).await;

    // A parada não pode herdar o `?`: se ela falhar, a câmera fica girando e o erro
    // certo a mostrar ainda é o do movimento. Registrar e seguir é o mal menor.
    if let Err(erro) = parar(http, host, perfil).await {
        eprintln!("[jarvis] a câmera {host} não confirmou a parada do PTZ: {erro}");
    }

    Ok(())
}

pub async fn parar(http: &reqwest::Client, host: &str, perfil: &str) -> Result<(), OnvifError> {
    let corpo = format!(
        "<tptz:Stop><tptz:ProfileToken>{perfil}</tptz:ProfileToken>\
         <tptz:PanTilt>true</tptz:PanTilt></tptz:Stop>"
    );

    falar(http, host, &corpo).await.map(|_| ())
}

/// O caminho que a família da V380 usa, para quando ninguém perguntou à câmera.
///
/// É palpite, e está marcado como tal: o certo é gravar no cadastro o que o
/// [`stream_uri`] devolveu. Existe para o cadastro manual funcionar sem uma ida à rede.
///
/// **`ch00_1` e não `ch00_0`, e a numeração engana.** Medido contra o hardware: o
/// `ch00_0` é o SECUNDÁRIO (640×480) e o `ch00_1` é o principal (1280×720) — o inverso do
/// que o número sugere. Errar aqui não dá erro nenhum, só uma imagem em qualidade baixa
/// que ninguém liga ao palpite que a produziu.
pub fn rtsp_provavel(host: &str, usuario: &str, senha: &str) -> String {
    com_credenciais(&format!("rtsp://{host}/live/ch00_1"), usuario, senha)
}

/// Injeta `usuario:senha@` numa URL RTSP que não os tem.
///
/// A câmera devolve a URL sem credencial mesmo quando exige uma — o `GetStreamUri` diz
/// ONDE está o stream, não como entrar. Sem esta injeção, o go2rtc leva 401 numa URL que
/// veio da própria câmera, que é o tipo de erro que faz procurar defeito no lugar errado.
///
/// URL que já traz credencial passa intacta: reescrever a que o usuário digitou à mão
/// seria ignorar a única fonte que sabe mais que nós.
pub fn com_credenciais(url: &str, usuario: &str, senha: &str) -> String {
    let Some(resto) = url.strip_prefix("rtsp://") else {
        return url.to_owned();
    };

    // Um `@` antes da primeira barra é credencial que já está lá.
    let tem_credencial = resto
        .split('/')
        .next()
        .is_some_and(|autoridade| autoridade.contains('@'));

    if usuario.is_empty() || tem_credencial {
        return url.to_owned();
    }

    format!(
        "rtsp://{}:{}@{resto}",
        super::xiongmai::escapar(usuario),
        super::xiongmai::escapar(senha)
    )
}

/// O conteúdo de `<algo:nome>…</…>`, com as entidades XML resolvidas.
///
/// Aceita com e sem prefixo de namespace porque as duas formas aparecem: a resposta usa
/// `<tt:Uri>`, mas nem toda câmera prefixa.
fn campo(xml: &str, nome: &str) -> Option<String> {
    let com_ns = format!(":{nome}>");
    let sem_ns = format!("<{nome}>");

    let inicio = xml
        .find(&com_ns)
        .map(|i| i + com_ns.len())
        .or_else(|| xml.find(&sem_ns).map(|i| i + sem_ns.len()))?;

    let resto = &xml[inicio..];
    let fim = resto.find("</")?;

    let valor = decodificar(resto[..fim].trim());
    (!valor.is_empty()).then_some(valor)
}

/// Os `token="…"` que estão dentro de um elemento `Profiles`.
///
/// **Só os dos perfis.** A resposta traz tokens de configuração junto
/// (`VideoSourceConfiguration0`, `Anv_ptz_0`), e pegar `token="` solto no XML devolveria
/// os oito misturados — com o primeiro deles funcionando por acaso e o resto dando erro
/// de perfil inexistente.
fn tokens_de_perfil(xml: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    for pedaco in xml.split('<').filter(|p| {
        // `Profiles ` com espaço: é a tag de abertura com atributos, e não a
        // `GetProfilesResponse` que a contém.
        p.starts_with("trt:Profiles ") || p.starts_with("Profiles ")
    }) {
        let Some(depois) = pedaco.split_once("token=\"") else {
            continue;
        };
        let Some((valor, _)) = depois.1.split_once('"') else {
            continue;
        };

        tokens.push(valor.to_owned());
    }

    tokens
}

/// Resolve as entidades XML.
///
/// **É o `&amp;` que importa.** A URL do DVR carrega `&` entre os parâmetros, e uma
/// resposta ONVIF a devolve escapada. Entregar isso cru ao go2rtc produz uma URL com
/// `&amp;` no meio — que não dá erro de parse, só nunca conecta.
fn decodificar(bruto: &str) -> String {
    bruto
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resposta real da câmera, encurtada.
    const PERFIS: &str = r#"<SOAP-ENV:Envelope><SOAP-ENV:Body><trt:GetProfilesResponse>
        <trt:Profiles fixed="true" token="stream0_0"><tt:Name>stream0_0</tt:Name>
        <tt:VideoSourceConfiguration token="VideoSourceConfiguration0"/>
        <tt:PTZConfiguration token="Anv_ptz_0"/></trt:Profiles>
        <trt:Profiles fixed="true" token="stream0_1">
        <tt:VideoEncoderConfiguration token="VideoEncoderConfiguration0_1"/>
        </trt:Profiles></trt:GetProfilesResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;

    /// Pegar `token="` solto traria os seis tokens de configuração junto — e o erro
    /// apareceria só como "perfil inexistente" na segunda câmera.
    #[test]
    fn le_so_os_tokens_dos_perfis() {
        assert_eq!(tokens_de_perfil(PERFIS), ["stream0_0", "stream0_1"]);
    }

    #[test]
    fn le_a_uri_com_prefixo_de_namespace() {
        let xml = "<trt:MediaUri><tt:Uri>rtsp://192.168.18.179/live/ch00_0</tt:Uri></trt:MediaUri>";
        assert_eq!(
            campo(xml, "Uri").unwrap(),
            "rtsp://192.168.18.179/live/ch00_0"
        );
    }

    /// Nem toda câmera prefixa; as duas formas têm que passar.
    #[test]
    fn le_a_uri_sem_prefixo() {
        assert_eq!(campo("<Uri>rtsp://x/y</Uri>", "Uri").unwrap(), "rtsp://x/y");
    }

    /// O `&amp;` cru produz uma URL que não dá erro de parse e nunca conecta — o modo
    /// de falha mais caro deste módulo.
    #[test]
    fn resolve_o_e_comercial_escapado() {
        let xml = "<tt:Uri>rtsp://h/user=admin&amp;channel=1&amp;stream=0.sdp?</tt:Uri>";
        assert_eq!(
            campo(xml, "Uri").unwrap(),
            "rtsp://h/user=admin&channel=1&stream=0.sdp?"
        );
    }

    #[test]
    fn campo_ausente_e_none() {
        assert!(campo("<tt:Outro>x</tt:Outro>", "Uri").is_none());
        // Presente mas vazio conta como ausente: uma URL vazia não serve para nada, e
        // é assim que o `GetSnapshotUri` desta câmera responde.
        assert!(campo("<tt:Uri></tt:Uri>", "Uri").is_none());
    }

    #[test]
    fn injeta_credencial_em_url_sem_ela() {
        assert_eq!(
            com_credenciais("rtsp://10.0.0.5/live/ch00_0", "admin", "1234"),
            "rtsp://admin:1234@10.0.0.5/live/ch00_0"
        );
    }

    /// A URL que já veio com credencial é a fonte que sabe mais que nós.
    #[test]
    fn nao_mexe_em_url_que_ja_tem_credencial() {
        let url = "rtsp://joao:senha@10.0.0.5/live";
        assert_eq!(com_credenciais(url, "admin", "outra"), url);
    }

    /// Câmera sem senha (a V380 é uma) não pode ganhar um `@` solto.
    #[test]
    fn sem_usuario_a_url_passa_intacta() {
        let url = "rtsp://10.0.0.5/live/ch00_0";
        assert_eq!(com_credenciais(url, "", ""), url);
    }

    /// Uma barra depois do host não pode ser confundida com credencial ausente — e um
    /// `@` no CAMINHO não é credencial.
    #[test]
    fn arroba_no_caminho_nao_conta_como_credencial() {
        assert_eq!(
            com_credenciais("rtsp://10.0.0.5/live@1", "admin", "x"),
            "rtsp://admin:x@10.0.0.5/live@1"
        );
    }

    #[test]
    fn cada_direcao_tem_seu_vetor() {
        assert_eq!(Direcao::Esquerda.vetor().0, -VELOCIDADE);
        assert_eq!(Direcao::Direita.vetor().0, VELOCIDADE);
        // Cima é `y` positivo, pela convenção da especificação.
        assert_eq!(Direcao::Cima.vetor().1, VELOCIDADE);
        assert_eq!(Direcao::Baixo.vetor().1, -VELOCIDADE);
    }
}
