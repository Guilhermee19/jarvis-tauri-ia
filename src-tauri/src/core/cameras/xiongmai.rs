//! A URL RTSP de um DVR Xiongmai — o aparelho que o app XMEye abre.
//!
//! Estes DVRs se anunciam como `Server: H264DVR 1.0` e servem uma interface web
//! chamada `NETSurveillance`. Eles falam RTSP na 554 com autenticação Digest, mas o
//! caminho da URL **não** é o `/cam/realmonitor` da Dahua nem o `/onvif1` genérico: é
//! um formato próprio que repete usuário e senha como parâmetros de query.
//!
//! ```text
//! rtsp://admin:1234@192.168.18.249:554/user=admin&password=1234&channel=1&stream=0.sdp?
//! ```
//!
//! **A credencial aparece duas vezes de propósito.** A do `userinfo` (antes do `@`) é a
//! que o Digest usa; a da query é a que o firmware lê para escolher o canal. Mandar só
//! uma das duas dá 401 em parte dos firmwares e um stream preto no resto — e as duas
//! falhas se parecem com "senha errada", que é o palpite errado.
//!
//! **A interrogação no final também não é enfeite.** O caminho termina em `.sdp?` com a
//! query vazia depois; sem ela, firmwares dessa família devolvem 404. É copiado do que o
//! próprio app faz, não deduzido.
//!
//! Um DVR é multi-canal: `channel` começa em **1** e cada valor é uma câmera física
//! diferente. `stream=0` é o principal (resolução cheia) e `stream=1` é o secundário,
//! que é o que se quer quando a imagem serve para vigiar em vez de assistir.

/// O canal principal, em resolução cheia. É o que vai para a tela e para a visão.
///
/// Não existe um par `rtsp_sub` aqui, embora o firmware ofereça `stream=1`. Ele serviria
/// ao [`super::vigia`], que só precisa de 64×48 — mas o gargalo da vigilância é o modelo
/// de visão, não a banda de uma rede local, e ter um segundo stream por câmera obrigaria
/// a derivar o secundário de uma URL que o usuário pode ter digitado à mão. O ganho não
/// paga o caminho frágil; se um dia a conta virar, o lugar é aqui.
pub fn rtsp(host: &str, canal: u8, usuario: &str, senha: &str) -> String {
    montar(host, canal, usuario, senha, 0)
}

fn montar(host: &str, canal: u8, usuario: &str, senha: &str, stream: u8) -> String {
    // Canal 0 não existe neste firmware, e o erro dele é um stream que abre e nunca
    // manda quadro — pior que um 404, porque parece problema de rede.
    let canal = canal.max(1);
    let credencial = credencial(usuario, senha);

    format!(
        "rtsp://{credencial}{host}:554/user={u}&password={s}&channel={canal}&stream={stream}.sdp?",
        u = escapar(usuario),
        s = escapar(senha),
    )
}

/// O `usuario:senha@` do começo da URL. Vazio quando não há usuário — um DVR sem senha
/// recusa a URL se ela vier com um `@` solto na frente.
fn credencial(usuario: &str, senha: &str) -> String {
    if usuario.is_empty() {
        return String::new();
    }

    format!("{}:{}@", escapar(usuario), escapar(senha))
}

/// Percent-encode do que não pode passar cru.
///
/// A senha é o problema: um `@` nela parte a URL no lugar errado e o host vira a metade
/// de trás da senha — o erro sai como "não resolvi o endereço", que manda procurar na
/// rede um defeito que está no cadastro. `&` e `=` quebram a query do mesmo jeito.
///
/// Visível para o [`super::onvif`], que precisa do mesmo escape ao injetar credencial
/// numa URL que a câmera devolveu.
pub(super) fn escapar(bruto: &str) -> String {
    let mut saida = String::with_capacity(bruto.len());

    for c in bruto.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => saida.push(c),
            outro => {
                let mut buf = [0u8; 4];
                for byte in outro.encode_utf8(&mut buf).as_bytes() {
                    saida.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }

    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monta_a_url_do_formato_do_firmware() {
        assert_eq!(
            rtsp("192.168.18.249", 1, "admin", "1234"),
            "rtsp://admin:1234@192.168.18.249:554/user=admin&password=1234&channel=1&stream=0.sdp?"
        );
    }

    /// O canal entra na URL como foi pedido, e o stream é sempre o principal.
    #[test]
    fn o_canal_passa_e_o_stream_e_o_principal() {
        let terceiro = rtsp("10.0.0.5", 3, "admin", "x");
        assert!(terceiro.contains("channel=3"));
        assert!(terceiro.ends_with("stream=0.sdp?"));
    }

    /// Canal 0 abre um stream que nunca manda quadro — parece problema de rede, e não
    /// é. Corrigir aqui é mais barato que descobrir isso na frente da câmera.
    #[test]
    fn canal_zero_vira_um() {
        assert!(rtsp("10.0.0.5", 0, "admin", "x").contains("channel=1"));
    }

    /// Um `@` na senha partiria a URL e o host viraria a metade de trás dela.
    #[test]
    fn escapa_o_que_quebraria_a_url() {
        let url = rtsp("10.0.0.5", 1, "admin", "a@b&c=d");

        assert!(url.contains("admin:a%40b%26c%3Dd@10.0.0.5"));
        assert!(url.contains("password=a%40b%26c%3Dd"));
        // Depois do escape sobra UM `@`, o que separa credencial de host.
        assert_eq!(url.matches('@').count(), 1);
    }

    /// DVR sem senha nenhuma não pode receber um `@` solto na frente do host.
    #[test]
    fn sem_usuario_nao_sobra_arroba() {
        let url = rtsp("10.0.0.5", 1, "", "");

        assert!(url.starts_with("rtsp://10.0.0.5:554/"));
        assert!(!url.contains('@'));
    }

    /// Acento em senha é legal e o byte cru não passa numa URL.
    #[test]
    fn escapa_fora_do_ascii() {
        assert!(rtsp("10.0.0.5", 1, "admin", "senhá").contains("password=senh%C3%A1"));
    }
}
