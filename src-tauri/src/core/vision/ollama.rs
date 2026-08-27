//! Visão pelo modelo local, no MESMO Ollama já quente do intérprete.
//!
//! É o caminho padrão e o fallback: sem chave da Anthropic ele é o único, e com chave
//! ele é quem responde quando a rede cai.

use super::{prompt, schema, AgentError, Fonte, Imagem, Visao};

pub async fn ver(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    imagem: &Imagem<'_>,
    pergunta: &str,
    fonte: Fonte,
) -> Result<Visao, AgentError> {
    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": crate::core::agent::KEEP_ALIVE,
        // O mesmo `format` que o roteador usa: vira grammar no llama.cpp, então o
        // modelo não CONSEGUE devolver prosa solta em vez do objeto.
        "format": schema(),
        // Baixa: dizer o que está na imagem é observação, não criação.
        "options": { "temperature": 0.2, "num_predict": 250 },
        "messages": [{
            "role": "user",
            "content": prompt(pergunta, fonte),
            // O Ollama quer base64 puro aqui, e ignora o mime — ele infere do conteúdo.
            "images": [imagem.base64],
        }],
    });

    let texto = crate::core::agent::pedir_ao_modelo(http, url, model, &corpo).await?;
    let texto = texto.trim();

    // Modelo de texto puro ignora o campo `images` e responde do nada; modelo de visão
    // pequeno devolve vazio quando não entende o idioma. Os dois casos chegam aqui como
    // resposta inútil, e é melhor dizer o que houve do que inventar uma descrição.
    if texto.is_empty() {
        return Err(AgentError::SemVisao(model.to_owned()));
    }

    let visao: Visao =
        serde_json::from_str(texto).map_err(|_| AgentError::NaoEntendi(recorte(texto)))?;

    // Schema cumprido e `resposta` vazia é o mesmo sintoma de antes, só que um nível
    // mais fundo — vale a mesma mensagem, que diz como trocar o modelo.
    if visao.resposta.trim().is_empty() {
        return Err(AgentError::SemVisao(model.to_owned()));
    }

    Ok(visao)
}

fn recorte(texto: &str) -> String {
    texto.chars().take(200).collect()
}
