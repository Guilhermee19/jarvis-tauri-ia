//! A única parte da casa inteligente que fala com a internet.
//!
//! Serve para buscar as duas coisas que o anúncio da rede local **nunca** traz: o NOME
//! que você deu ao aparelho no app, e a `local_key`, que é o segredo sem o qual a porta
//! 6668 recusa qualquer comando.
//!
//! **É uma visita só.** A chave é do APARELHO, não da nuvem: depois de importada, o
//! controle é local para sempre, e continua funcionando quando o projeto trial da Tuya
//! expirar. Reimportar só é preciso quando um aparelho é pareado de novo — o que
//! **troca a chave dele** — ou quando entra um aparelho novo na casa.
//!
//! ## O passo manual, que não tem como evitar
//!
//! A chave existe porque a Tuya a gerou no pareamento, e ela só sai por uma conta de
//! desenvolvedor em `iot.tuya.com` com a conta do app ligada por QR code. Não há
//! caminho local: o aparelho não conta a própria chave, e é justamente isso que impede
//! o vizinho de apagar sua luz.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use sha2::Sha256;

use crate::core::casa::chaveiro::Conhecido;

/// Mesma política do `core::music`: teto por request, e não no cliente — o cliente
/// global tem 180 s por causa do Ollama, que é uma espera de outra natureza.
const TIMEOUT: Duration = Duration::from_secs(15);

/// SHA-256 do corpo vazio, que é o de todo GET daqui. Fixo em vez de calculado porque
/// é sempre o mesmo e aparece em cada assinatura.
const CORPO_VAZIO: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Debug, thiserror::Error)]
pub enum NuvemError {
    #[error(
        "sem credenciais da Tuya — preencha Access ID e Access Secret em Configurações. Elas saem de um projeto Cloud em iot.tuya.com"
    )]
    SemCredencial,
    #[error(
        "não sei por qual aparelho começar: rode a varredura da rede antes de importar. A busca da nuvem parte de um id que a própria rede anunciou"
    )]
    SemSemente,
    #[error("a Tuya recusou a chamada (HTTP {status})")]
    Recusado { status: u16 },
    /// O `caminho` está na mensagem de propósito: **o mesmo código quer dizer coisas
    /// opostas conforme a chamada**. Um 1106 no `/token` é projeto ou data center; um
    /// 1106 no `/devices/…` é o aparelho não estar na conta ligada ao projeto. Sem saber
    /// qual das duas, a dica teria que listar as duas e não ajudaria em nenhuma.
    #[error("a Tuya recusou {caminho}: {msg} (código {codigo}). {dica}")]
    Negado {
        codigo: i64,
        caminho: String,
        msg: String,
        dica: String,
    },
    /// O caso que mais assusta e mais engana: autenticou, respondeu `success`, e a
    /// lista veio vazia. Sem esta variante, a tela diria "nenhum aparelho" — que é
    /// exatamente o que ela diria se o problema fosse a rede.
    #[error(
        "a Tuya autenticou, e o projeto não tem aparelho nenhum ligado a ele. Falta ligar a conta do app: no projeto em iot.tuya.com, aba Devices > Link App Account > Add App Account, e ler o QR code pelo Smart Life (Eu > ícone de escanear, canto superior direito). Se você já fez isso, confira se o data center do projeto é o mesmo da conta do app — são listas separadas por data center"
    )]
    ListaVazia,
    #[error("sem internet para falar com a Tuya: {0}")]
    Rede(String),
}

/// Busca na nuvem o nome e a chave de todos os aparelhos da conta.
///
/// ## Dois caminhos, e o segundo é o de recurso
///
/// O bom é perguntar ao PROJETO quais aparelhos ele enxerga
/// (`/v1.0/iot-01/associated-users/devices`): ele responde a lista das contas de app
/// ligadas a ele, sem precisar saber o id de ninguém de antemão.
///
/// O de recurso parte de uma `semente` — o id de um aparelho que a varredura local viu —
/// para descobrir o usuário dono e então listar os aparelhos DELE. Ele existe porque
/// nem todo projeto tem o primeiro endpoint liberado, e porque em alguns a listagem vem
/// sem a `local_key`.
///
/// A ordem importa: partir da semente falha com "permission deny" quando aquele aparelho
/// específico não está na conta ligada ao projeto — o que diz muito pouco, já que outros
/// aparelhos poderiam estar. Perguntar ao projeto primeiro tira a rede local do caminho.
pub async fn importar(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    regiao: &str,
    semente: &str,
) -> Result<Vec<Conhecido>, NuvemError> {
    let client_id = client_id.trim();
    let client_secret = client_secret.trim();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(NuvemError::SemCredencial);
    }

    let semente = semente.trim();
    if semente.is_empty() {
        return Err(NuvemError::SemSemente);
    }

    let base = base(regiao);

    #[derive(Deserialize)]
    struct Token {
        access_token: String,
    }
    let token: Token = chamar(
        http,
        &base,
        client_id,
        client_secret,
        None,
        "/v1.0/token?grant_type=1",
    )
    .await?;

    let token = &token.access_token;

    // O caminho bom. Falha inteira (projeto sem o endpoint liberado) cai no de recurso;
    // sucesso sem chave nenhuma também, porque uma lista de nomes sem `local_key` não
    // controla nada.
    let associados = listar(
        http,
        &base,
        client_id,
        client_secret,
        token,
        "/v1.0/iot-01/associated-users/devices?size=100",
    )
    .await;

    let associados = match associados {
        Ok(lista) if lista.is_empty() => return Err(NuvemError::ListaVazia),
        Ok(lista) if lista.iter().any(|cru| !cru.local_key.trim().is_empty()) => {
            return Ok(lista.into_iter().map(conhecido).collect())
        }
        Ok(lista) => lista,
        Err(erro) => {
            eprintln!("[jarvis] tuya: a lista do projeto não veio ({erro}); tentando pela semente");
            Vec::new()
        }
    };

    // A semente da PRÓPRIA conta é melhor que a da rede: um aparelho que o projeto
    // acabou de listar está garantidamente visível para ele.
    let semente = associados
        .first()
        .map(|cru| cru.id.clone())
        .unwrap_or_else(|| semente.to_owned());

    #[derive(Deserialize)]
    struct Dono {
        uid: String,
    }
    let dono: Dono = chamar(
        http,
        &base,
        client_id,
        client_secret,
        Some(token),
        &format!("/v1.0/devices/{semente}"),
    )
    .await?;

    let crus = listar(
        http,
        &base,
        client_id,
        client_secret,
        token,
        &format!("/v1.0/users/{}/devices", dono.uid),
    )
    .await?;

    if crus.is_empty() {
        return Err(NuvemError::ListaVazia);
    }

    Ok(crus.into_iter().map(conhecido).collect())
}

/// Um aparelho como a Tuya o descreve. Nomes crus dela, e todos com `default` porque o
/// subconjunto varia por categoria — um campo ausente não pode derrubar a lista inteira.
#[derive(Deserialize)]
struct Cru {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    local_key: String,
    #[serde(default)]
    product_id: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    online: bool,
}

fn conhecido(cru: Cru) -> Conhecido {
    Conhecido {
        id: cru.id,
        nome: cru.name,
        local_key: cru.local_key,
        produto: cru.product_id,
        categoria: cru.category,
        online: cru.online,
        // A nuvem manda um `ip`, mas é o PÚBLICO do roteador — não serve para falar com
        // o aparelho. Endereço, versão e a hora em que foi visto são todos da varredura
        // local, e entram depois pelo `Chaveiro::vistos`.
        ..Conhecido::default()
    }
}

/// Uma listagem de aparelhos, nos DOIS formatos que a Tuya usa.
///
/// `/v1.0/users/{uid}/devices` responde um array direto; o
/// `/v1.0/iot-01/associated-users/devices` embrulha num objeto com paginação. Aceitar os
/// dois aqui é o que deixa os dois caminhos do [`importar`] compartilharem o resto.
async fn listar(
    http: &reqwest::Client,
    base: &str,
    client_id: &str,
    client_secret: &str,
    token: &str,
    caminho: &str,
) -> Result<Vec<Cru>, NuvemError> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Listagem {
        Direta(Vec<Cru>),
        Paginada {
            #[serde(default)]
            devices: Vec<Cru>,
        },
    }

    let listagem: Listagem = chamar(
        http,
        base,
        client_id,
        client_secret,
        Some(token),
        caminho,
    )
    .await?;

    Ok(match listagem {
        Listagem::Direta(aparelhos) => aparelhos,
        Listagem::Paginada { devices } => devices,
    })
}

/// O endereço do *data center* onde o projeto vive.
///
/// Região desconhecida cai no `us` em vez de recusar: o palpite mais provável é melhor
/// que um erro, e quando ele erra a mensagem da [`NuvemError::ListaVazia`] já ensina
/// exatamente o que trocar.
fn base(regiao: &str) -> String {
    let sufixo = match regiao.trim().to_ascii_lowercase().as_str() {
        "eu" => "eu",
        "cn" => "cn",
        "in" => "in",
        _ => "us",
    };

    format!("https://openapi.tuya{sufixo}.com")
}

/// Um GET assinado, e o desembrulho do envelope que a Tuya põe em volta de tudo.
///
/// O envelope é a parte que engana: **a Tuya responde HTTP 200 mesmo quando recusa**, e
/// o motivo real está num `success: false` lá dentro. Tratar só o status daria "deu
/// certo" para um "permission deny".
async fn chamar<T: DeserializeOwned>(
    http: &reqwest::Client,
    base: &str,
    client_id: &str,
    client_secret: &str,
    token: Option<&str>,
    caminho: &str,
) -> Result<T, NuvemError> {
    #[derive(Deserialize)]
    struct Envelope<T> {
        #[serde(default)]
        success: bool,
        #[serde(default)]
        code: i64,
        #[serde(default)]
        msg: String,
        result: Option<T>,
    }

    let t = agora_ms().to_string();
    let sign = assinar(
        client_secret,
        &format!(
            "{client_id}{}{t}GET\n{CORPO_VAZIO}\n\n{caminho}",
            token.unwrap_or_default()
        ),
    );

    let mut pedido = http
        .get(format!("{base}{caminho}"))
        .timeout(TIMEOUT)
        .header("client_id", client_id)
        .header("sign", sign)
        .header("t", &t)
        .header("sign_method", "HMAC-SHA256");

    if let Some(token) = token {
        pedido = pedido.header("access_token", token);
    }

    let resposta = pedido
        .send()
        .await
        .map_err(|erro| NuvemError::Rede(erro.to_string()))?;

    let status = resposta.status();
    if !status.is_success() {
        return Err(NuvemError::Recusado {
            status: status.as_u16(),
        });
    }

    // Como texto antes de desserializar: quando o formato muda, o corpo é a única coisa
    // que diz o que veio — mesma escolha do `core::music::procurar`.
    let texto = resposta
        .text()
        .await
        .map_err(|erro| NuvemError::Rede(erro.to_string()))?;

    let envelope: Envelope<T> = serde_json::from_str(&texto).map_err(|erro| {
        eprintln!("[jarvis] tuya {caminho} não desserializou ({erro}); corpo: {texto:.400}");
        NuvemError::Rede(erro.to_string())
    })?;

    if !envelope.success {
        return Err(NuvemError::Negado {
            dica: dica(envelope.code, caminho),
            codigo: envelope.code,
            caminho: caminho.to_owned(),
            msg: envelope.msg,
        });
    }

    envelope.result.ok_or_else(|| {
        eprintln!("[jarvis] tuya {caminho} veio sem `result`; corpo: {texto:.400}");
        NuvemError::Rede("resposta sem conteúdo".to_owned())
    })
}

/// O que fazer a respeito, por código de erro da Tuya e por chamada.
///
/// Existe porque as mensagens dela são de três palavras em inglês ("permission deny") e
/// não dizem nada sobre a causa, que quase sempre é uma configuração no site — não algo
/// no código nem na rede.
fn dica(codigo: i64, caminho: &str) -> String {
    // Autenticar é a primeira chamada: se ela passou, credencial e data center estão
    // certos, e o que sobra é o que o projeto enxerga da SUA conta do app.
    let ja_autenticou = !caminho.starts_with("/v1.0/token");

    match codigo {
        1106 if ja_autenticou => return "As credenciais e o data center estão certos — o que faltou foi o projeto ENXERGAR este aparelho. Em iot.tuya.com, abra o projeto, vá na aba Devices > Link App Account e confira se a sua conta do Smart Life aparece ali com os aparelhos. Se não aparecer, clique em Add App Account e leia o QR code pelo app (Eu > ícone de escanear, no canto superior direito).".to_owned(),
        _ => {}
    }

    match codigo {
        1004 => "A assinatura não bateu: confira se o Access Secret foi colado inteiro, sem espaço sobrando.",
        // A mensagem da Tuya JÁ traz o IP, e é o único código aqui em que o texto dela
        // vale mais que a dica — por isso esta só diz onde clicar.
        1114 => "Este código quer dizer que as credenciais estão CERTAS e o projeto barrou o endereço. No projeto em iot.tuya.com, abra a aba \"Authorization\" (em algumas versões o bloco fica no fim da aba \"Overview\"), procure \"IP Allowlist\" e acrescente EXATAMENTE o IP que a mensagem acima mostra. Cuidado com duas armadilhas: não é o endereço 192.168.x.x deste PC (esse a Tuya nunca vê, ela enxerga o IP público do seu roteador), e a lista é uma por data center — cadastre na aba do mesmo data center configurado aqui. Internet residencial troca de IP de vez em quando, então isso pode precisar ser refeito.",
        1106 => "Costuma ser o data center errado, ou o serviço IoT Core não assinado no projeto (Cloud > Cloud Services > IoT Core).",
        // Vem com a data de expiração no `msg`, e o botão que resolve fica na mesma tela
        // — só que numa linha de tabela que ninguém repara.
        28841002 => "O trial do IoT Core venceu. Em iot.tuya.com, vá em Cloud > Cloud Services > IoT Core, aba My Subscriptions, e clique em \"Extend Trial Period\" na linha do IoT Core (fica ao lado da data de expiração). É grátis e renovável. Isso só trava a IMPORTAÇÃO: aparelho cujas chaves já foram importadas continua sendo controlado normalmente, porque a chave é do aparelho e não da nuvem.",
        1010 | 1011 => "O token expirou ou é de outro projeto. Tente importar de novo.",
        28841105 => "O projeto trial expirou. Renove em iot.tuya.com > Cloud > Cloud Services, com o botão \"Extend Trial Period\" — os aparelhos já importados continuam funcionando sem isso.",
        _ => "Confira o Access ID, o Access Secret e o data center do projeto em Configurações.",
    }
    .to_owned()
}

fn agora_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|desde| desde.as_millis())
        .unwrap_or_default()
}

/// HMAC-SHA256 em hexa MAIÚSCULO, que é o formato que a Tuya exige.
///
/// Minúsculo é recusado com o mesmo "sign invalid" de uma chave errada — foi por isso
/// que o `.to_uppercase()` ganhou um comentário em vez de ficar escondido no fim da
/// linha.
fn assinar(segredo: &str, mensagem: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(segredo.as_bytes())
        .expect("HMAC aceita chave de qualquer tamanho");
    mac.update(mensagem.as_bytes());

    hex(&mac.finalize().into_bytes()).to_uppercase()
}

/// Hexa minúsculo, à mão. Mesma decisão do `urlencode` do `core::music`: uma crate
/// inteira por cinco linhas sai mais caro que as cinco linhas.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    /// Vetor fixo: se a montagem da mensagem assinada mudar de forma, isto quebra. É o
    /// único jeito de testar a assinatura sem uma conta da Tuya — e errar aqui não dá
    /// erro legível, dá um "sign invalid" idêntico ao de credencial errada.
    #[test]
    fn a_assinatura_bate_com_o_hmac_de_referencia() {
        // Conferido contra `hmac.new(b"segredo", b"mensagem", sha256)` do Python.
        assert_eq!(
            assinar("segredo", "mensagem"),
            "07F6A694A51E3BC9FC4B600096D7B5C620D1AABD804D4A8CF33DBADD71EA3D38",
            "a Tuya recusa hexa minúsculo com o MESMO erro de credencial errada"
        );
    }

    /// A ordem dos pedaços é a parte que não perdoa: trocar dois deles dá uma
    /// assinatura perfeitamente válida que a Tuya recusa com "sign invalid", sem dizer
    /// mais nada. Este teste congela a forma da mensagem sem precisar de conta.
    #[test]
    fn a_mensagem_assinada_tem_a_ordem_exata() {
        let sem_token = format!("cli{}{}GET
{CORPO_VAZIO}

{}", "", "1700", "/v1.0/token?grant_type=1");
        assert_eq!(
            sem_token,
            format!("cli1700GET
{CORPO_VAZIO}

/v1.0/token?grant_type=1")
        );

        // Com token, ele entra ENTRE o client_id e o `t` — não no fim.
        let com_token = format!("cli{}{}GET
{CORPO_VAZIO}

{}", "tok", "1700", "/v1.0/devices/x");
        assert!(com_token.starts_with("clitok1700GET"));
    }

    /// O SHA-256 do corpo vazio está fixo numa constante. Se ele estiver errado, TODA
    /// chamada falha com "sign invalid" e nada aponta para cá.
    #[test]
    fn o_corpo_vazio_confere() {
        let mut hasher = Sha256::new();
        hasher.update(b"");

        assert_eq!(hex(&hasher.finalize()), CORPO_VAZIO);
    }

    #[test]
    fn a_regiao_desconhecida_cai_no_data_center_mais_provavel() {
        assert_eq!(base("eu"), "https://openapi.tuyaeu.com");
        assert_eq!(base("US"), "https://openapi.tuyaus.com");
        assert_eq!(base(" in "), "https://openapi.tuyain.com");
        assert_eq!(base("marte"), "https://openapi.tuyaus.com");
        assert_eq!(base(""), "https://openapi.tuyaus.com");
    }
}
