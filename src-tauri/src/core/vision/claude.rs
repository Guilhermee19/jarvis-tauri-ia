//! Visão pela Messages API da Anthropic.
//!
//! HTTP cru e não SDK porque não existe SDK oficial em Rust — e porque é o padrão da
//! casa: Ollama, ElevenLabs, Brave e Spotify já são `reqwest` direto.
//!
//! Todo erro daqui é engolido pelo `super::ver`, que cai no modelo local. Por isso as
//! mensagens são de diagnóstico (vão para o stderr), não texto de tela.

use std::time::Duration;

use serde::Deserialize;

use super::{prompt, schema, AgentError, Fonte, Imagem, Visao};

const API: &str = "https://api.anthropic.com/v1/messages";

/// Fixa no código, como o `MODEL_ID` do TTS: escolher modelo de visão não é decisão de
/// quem usa o app, e um campo a mais nas configurações seria mais uma coisa para
/// digitar errado. Trocar por `claude-sonnet-5` custa ~40% menos com a mesma resolução
/// máxima de imagem.
const MODELO: &str = "claude-opus-5";

/// A versão do formato da API, não do modelo. Obrigatória em toda chamada.
const VERSAO: &str = "2023-06-01";

/// Curto de propósito. Uma pergunta sobre imagem que passa disso já perdeu para o
/// modelo local, que responde em segundos — o cliente compartilhado tem 180 s porque
/// o Ollama carrega modelo na primeira chamada, e isso aqui não tem esse problema.
const TIMEOUT: Duration = Duration::from_secs(45);

pub async fn ver(
    http: &reqwest::Client,
    api_key: &str,
    imagem: &Imagem<'_>,
    pergunta: &str,
    fonte: Fonte,
) -> Result<Visao, AgentError> {
    let corpo = serde_json::json!({
        "model": MODELO,
        "max_tokens": 1024,
        "output_config": {
            // Descrever uma imagem não precisa de raciocínio profundo, e o tempo de
            // pensar entra INTEIRO na espera de quem perguntou.
            "effort": "low",
            "format": { "type": "json_schema", "schema": schema() },
        },
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": imagem.mime,
                        "data": imagem.base64,
                    },
                },
                { "type": "text", "text": prompt(pergunta, fonte) },
            ],
        }],
    });

    let resposta = http
        .post(API)
        .header("x-api-key", api_key)
        .header("anthropic-version", VERSAO)
        .timeout(TIMEOUT)
        .json(&corpo)
        .send()
        .await
        .map_err(rede)?;

    let status = resposta.status();
    let texto = resposta.text().await.map_err(rede)?;

    if !status.is_success() {
        return Err(AgentError::VisaoRemota(format!(
            "HTTP {}: {}",
            status.as_u16(),
            recorte(&texto)
        )));
    }

    let mensagem: Mensagem = serde_json::from_str(&texto)
        .map_err(|erro| AgentError::VisaoRemota(format!("resposta inesperada: {erro}")))?;

    // ANTES de ler `content`: numa recusa dos classificadores o array vem vazio, e
    // pegar o primeiro bloco entraria em pânico. Cai no modelo local, que não tem
    // classificador nenhum — a pergunta é respondida de um jeito ou de outro.
    if mensagem.stop_reason.as_deref() == Some("refusal") {
        return Err(AgentError::VisaoRemota(
            "a Anthropic recusou a imagem".into(),
        ));
    }

    let json = mensagem
        .content
        .into_iter()
        .find_map(|bloco| (bloco.tipo == "text").then_some(bloco.text))
        .ok_or_else(|| AgentError::VisaoRemota("resposta sem texto".into()))?;

    serde_json::from_str(&json).map_err(|_| {
        AgentError::VisaoRemota(format!("não é o objeto esperado: {}", recorte(&json)))
    })
}

#[derive(Deserialize)]
struct Mensagem {
    #[serde(default)]
    content: Vec<Bloco>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct Bloco {
    #[serde(rename = "type")]
    tipo: String,
    /// Blocos que não são de texto (`thinking`) não têm este campo.
    #[serde(default)]
    text: String,
}

fn rede(erro: reqwest::Error) -> AgentError {
    if erro.is_timeout() {
        return AgentError::VisaoRemota(format!("passou de {} s", TIMEOUT.as_secs()));
    }
    AgentError::VisaoRemota(erro.to_string())
}

fn recorte(texto: &str) -> String {
    texto.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_objeto_de_dentro_do_bloco_de_texto() {
        let bruto = r#"{"content":[{"type":"text","text":"{\"resposta\":\"um mouse\",\"buscar\":\"\"}"}],"stop_reason":"end_turn"}"#;
        let mensagem: Mensagem = serde_json::from_str(bruto).expect("envelope");

        let json = &mensagem.content[0].text;
        let visao: Visao = serde_json::from_str(json).expect("objeto");

        assert_eq!(visao.resposta, "um mouse");
        assert!(visao.buscar.is_empty());
    }

    /// Numa recusa o `content` vem VAZIO. Este teste existe porque o jeito natural de
    /// escrever o parse — pegar o primeiro bloco — entra em pânico exatamente aqui.
    #[test]
    fn recusa_vem_sem_conteudo_nenhum() {
        let bruto = r#"{"content":[],"stop_reason":"refusal"}"#;
        let mensagem: Mensagem = serde_json::from_str(bruto).expect("envelope");

        assert_eq!(mensagem.stop_reason.as_deref(), Some("refusal"));
        assert!(mensagem.content.is_empty());
    }

    /// Com `effort` ligado a resposta pode trazer um bloco de raciocínio na frente, e
    /// ele não tem campo `text`. Pegar o primeiro bloco pegaria o errado.
    #[test]
    fn pula_o_bloco_de_raciocinio_e_acha_o_texto() {
        let bruto =
            r#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"{}"}]}"#;
        let mensagem: Mensagem = serde_json::from_str(bruto).expect("envelope");

        let json = mensagem
            .content
            .into_iter()
            .find_map(|bloco| (bloco.tipo == "text").then_some(bloco.text));

        assert_eq!(json.as_deref(), Some("{}"));
    }
}
