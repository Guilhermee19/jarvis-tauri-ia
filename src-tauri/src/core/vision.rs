//! Olhar pela webcam e dizer o que está vendo.
//!
//! **Um modelo só para tudo, e isso foi medido.** A ideia óbvia era manter o
//! `qwen2.5:3b` para texto e somar um modelo de visão pequeno. Não funciona nesta
//! máquina: com 4 GB de VRAM o Ollama não segura os dois, e a primeira chamada ao
//! `moondream` levou **67 segundos** — o tempo de descarregar um e carregar o outro.
//! Como a mensagem seguinte é texto, ela pagaria a troca de volta. Cada "o que é
//! isso?" custaria dois minutos de vai e vem.
//!
//! Então o modelo do intérprete precisa ser multimodal, e a visão usa o MESMO modelo
//! já quente. Se o modelo configurado não enxergar, o erro diz isso em vez de
//! silenciar — foi outra coisa medida: perguntado em português, o `moondream`
//! devolveu string vazia em vez de recusar.

use super::agent::AgentError;

/// O `data:` URL que a captura devolve não serve para a API — ela quer só o base64.
///
/// Separado e testado porque o erro é silencioso: mandar o prefixo junto faz o modelo
/// receber lixo e descrever qualquer coisa, sem nunca reclamar.
pub fn so_o_base64(data_url: &str) -> &str {
    match data_url.split_once(',') {
        Some((cabecalho, dados)) if cabecalho.starts_with("data:") => dados,
        _ => data_url,
    }
}

/// Descreve a imagem em português, em uma ou duas frases.
pub async fn descrever(
    http: &reqwest::Client,
    url: &str,
    model: &str,
    imagem_base64: &str,
) -> Result<String, AgentError> {
    let corpo = serde_json::json!({
        "model": model,
        "stream": false,
        "keep_alive": super::agent::KEEP_ALIVE,
        // Baixa: descrever o que está na frente da câmera é observação, não criação.
        "options": { "temperature": 0.2, "num_predict": 250 },
        "messages": [{
            "role": "user",
            "content": PROMPT,
            "images": [imagem_base64],
        }],
    });

    let texto = super::agent::pedir_ao_modelo(http, url, model, &corpo).await?;
    let texto = texto.trim();

    // Modelo de texto puro ignora o campo `images` e responde do nada; modelo de visão
    // pequeno devolve vazio quando não entende o idioma. Os dois casos chegam aqui
    // como resposta inútil, e é melhor dizer o que houve do que inventar uma descrição.
    if texto.is_empty() {
        return Err(AgentError::SemVisao(model.to_owned()));
    }

    Ok(texto.to_owned())
}

const PROMPT: &str = "Olhe a imagem da webcam e responda EM PORTUGUÊS.

Em uma ou duas frases, diga o que você está vendo. Comece pelo objeto ou pela pessoa
principal, e só depois pelo cenário, se valer a pena.

Se estiver escuro, borrado ou sem nada reconhecível, diga isso — não invente.
Não descreva pixel por pixel e não comente a qualidade da imagem.";

#[cfg(test)]
mod tests {
    use super::*;

    /// Mandar o prefixo `data:` junto não dá erro: o modelo recebe lixo e descreve
    /// qualquer coisa. É o tipo de bug que só aparece como "ele viu errado".
    #[test]
    fn tira_o_prefixo_do_data_url() {
        assert_eq!(so_o_base64("data:image/jpeg;base64,AAAA"), "AAAA");
        assert_eq!(so_o_base64("data:image/png;base64,QUJD"), "QUJD");

        // Já sem prefixo, passa direto — e uma vírgula solta no base64 não engana.
        assert_eq!(so_o_base64("AAAA"), "AAAA");
        assert_eq!(so_o_base64("AA,AA"), "AA,AA");
    }
}
