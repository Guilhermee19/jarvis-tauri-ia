//! Transcrição local com o whisper.cpp.
//!
//! Fala com o `whisper-server.exe` por HTTP, e não com a biblioteca. Compilar o
//! `whisper-rs` exigiria CMake **e** LLVM/Clang instalados (o bindgen precisa do
//! `libclang`) — ~1,5 GB de toolchain no caminho de quem clonar o repo, para ganhar
//! o quê: o modelo residente. Como o servidor também mantém o modelo carregado entre
//! chamadas, o ganho é zero e o custo é alto. Quem sobe o processo é `core::services`.
//!
//! A conversa é a mesma forma do intérprete em `core::agent`: um POST, uma resposta.

use std::io::Cursor;
use std::path::Path;

use super::VoiceError;

/// O whisper.cpp RECUSA qualquer coisa que não seja 16 kHz — não reamostra sozinho.
const TAXA_DO_WHISPER: u32 = 16_000;

/// Abaixo disso é o botão tocado sem querer. Importa porque o Whisper alucina em
/// silêncio: a frase "Legendas pela comunidade Amara.org" é o exemplo clássico em
/// português, e ela viraria um comando.
const DURACAO_MINIMA_S: f32 = 0.4;

/// Lê o WAV que o microfone deixou, reamostra e transcreve.
pub async fn transcribe(
    http: &reqwest::Client,
    url: &str,
    wav: &Path,
) -> Result<String, VoiceError> {
    let audio = ler_e_reamostrar(wav)?;

    let parte = reqwest::multipart::Part::bytes(audio)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|error| VoiceError::TranscricaoRede(error.to_string()))?;

    let form = reqwest::multipart::Form::new()
        .part("file", parte)
        .text("response_format", "json")
        // Classificação de comando curto: nada de amostragem criativa.
        .text("temperature", "0.0");

    let endpoint = format!("{}/inference", url.trim_end_matches('/'));
    let resposta = http
        .post(&endpoint)
        .multipart(form)
        .send()
        .await
        .map_err(|error| {
            if error.is_connect() {
                VoiceError::TranscricaoOffline(url.to_owned())
            } else {
                VoiceError::TranscricaoRede(error.to_string())
            }
        })?;

    let status = resposta.status();
    if !status.is_success() {
        let corpo = resposta.text().await.unwrap_or_default();
        return Err(VoiceError::TranscricaoRecusada {
            status: status.as_u16(),
            corpo: corpo.chars().take(300).collect(),
        });
    }

    #[derive(serde::Deserialize)]
    struct Transcricao {
        text: String,
    }

    let transcricao: Transcricao = resposta
        .json()
        .await
        .map_err(|error| VoiceError::TranscricaoRede(error.to_string()))?;

    let texto = transcricao.text.trim().to_owned();
    if texto.is_empty() {
        return Err(VoiceError::NadaOuvido);
    }

    Ok(texto)
}

/// Devolve um WAV de 16 kHz pronto para o Whisper, em memória.
///
/// Em memória e não em disco porque o arquivo só existiria para ser lido de volta na
/// linha seguinte — o `stop_recording` já gravou o original, que continua sendo o que
/// a bancada de diagnóstico mostra.
fn ler_e_reamostrar(wav: &Path) -> Result<Vec<u8>, VoiceError> {
    let mut leitor =
        hound::WavReader::open(wav).map_err(|error| VoiceError::WavLeitura(error.to_string()))?;
    let spec = leitor.spec();

    let amostras: Vec<i16> = leitor
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|error| VoiceError::WavLeitura(error.to_string()))?;

    // O microfone grava mono (`mic.rs`), mas se um dia gravar estéreo o Whisper
    // receberia o dobro de amostras e o áudio sairia com o dobro da velocidade.
    let mono: Vec<i16> = if spec.channels > 1 {
        amostras.chunks(spec.channels as usize).map(media).collect()
    } else {
        amostras
    };

    let duracao = mono.len() as f32 / spec.sample_rate as f32;
    if duracao < DURACAO_MINIMA_S {
        return Err(VoiceError::GravacaoCurta);
    }

    let convertido = reamostrar(&mono, spec.sample_rate, TAXA_DO_WHISPER);
    escrever_wav(&convertido)
}

/// O microfone abre no formato nativo do dispositivo (`default_input_config`), que no
/// WASAPI compartilhado é tipicamente 48 kHz — nunca 16.
///
/// 48000 → 16000 é divisão exata por 3, e a média de cada trio é de quebra um filtro
/// passa-baixa de 3 taps: mais curto E melhor que interpolar.
///
/// ponytail: o caso não-inteiro (44100) cai na interpolação linear, sem filtro, o que
/// rebate o que está acima de 8 kHz de volta para dentro da banda. Para fala é inócuo
/// — o Whisper foi treinado em áudio de 16 kHz, já limitado em banda. Se a transcrição
/// sair suja num microfone de 44,1 kHz, aí entra um decimador de verdade.
fn reamostrar(entrada: &[i16], de: u32, para: u32) -> Vec<i16> {
    if de == para || entrada.is_empty() {
        return entrada.to_vec();
    }

    if de % para == 0 {
        let fator = (de / para) as usize;
        return entrada.chunks(fator).map(media).collect();
    }

    let razao = f64::from(de) / f64::from(para);
    let quantas = (entrada.len() as f64 / razao) as usize;
    let ultimo = entrada.len() - 1;

    (0..quantas)
        .map(|i| {
            let posicao = i as f64 * razao;
            let esquerda = (posicao as usize).min(ultimo);
            let direita = (esquerda + 1).min(ultimo);
            let peso = posicao - esquerda as f64;

            (f64::from(entrada[esquerda]) * (1.0 - peso) + f64::from(entrada[direita]) * peso)
                as i16
        })
        .collect()
}

/// Soma em `i32` antes de dividir: a soma de três amostras perto do pico estoura o
/// `i16` e a média sairia com o sinal trocado — um estalo no lugar da voz.
fn media(amostras: &[i16]) -> i16 {
    if amostras.is_empty() {
        return 0;
    }
    let soma: i32 = amostras.iter().map(|&a| i32::from(a)).sum();
    (soma / amostras.len() as i32) as i16
}

fn escrever_wav(amostras: &[i16]) -> Result<Vec<u8>, VoiceError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TAXA_DO_WHISPER,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut escritor = hound::WavWriter::new(&mut buffer, spec)
            .map_err(|error| VoiceError::WavWrite(error.to_string()))?;
        for &amostra in amostras {
            escritor
                .write_sample(amostra)
                .map_err(|error| VoiceError::WavWrite(error.to_string()))?;
        }
        escritor
            .finalize()
            .map_err(|error| VoiceError::WavWrite(error.to_string()))?;
    }

    Ok(buffer.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O caso real desta máquina: o microfone entrega 48 kHz e o Whisper exige 16.
    /// Errar isto não dá erro — dá transcrição de voz acelerada, que é bem pior de
    /// diagnosticar.
    #[test]
    fn reamostra_48k_para_16k_pela_media_de_cada_trio() {
        let entrada: Vec<i16> = (0..48_000).map(|i| (i % 100) as i16).collect();
        let saida = reamostrar(&entrada, 48_000, 16_000);

        assert_eq!(saida.len(), 16_000, "1 segundo tem que continuar 1 segundo");
        // Primeiro trio: (0 + 1 + 2) / 3 = 1.
        assert_eq!(saida[0], 1);
    }

    #[test]
    fn reamostra_taxa_nao_inteira_mantendo_a_duracao() {
        let entrada: Vec<i16> = (0..44_100).map(|i| (i % 100) as i16).collect();
        let saida = reamostrar(&entrada, 44_100, 16_000);

        assert_eq!(saida.len(), 16_000);
    }

    #[test]
    fn taxa_igual_nao_mexe_no_audio() {
        let entrada = vec![1_i16, 2, 3];
        assert_eq!(reamostrar(&entrada, 16_000, 16_000), entrada);
    }

    /// Sem o `i32` intermediário isto vira negativo e o áudio ganha um estalo.
    #[test]
    fn a_media_nao_estoura_perto_do_pico() {
        assert_eq!(media(&[i16::MAX, i16::MAX, i16::MAX]), i16::MAX);
        assert_eq!(media(&[10, 20, 30]), 20);
    }

    /// O WAV gerado precisa ser lido de volta pelo whisper.cpp — se o cabeçalho sair
    /// errado, o erro aparece lá, longe daqui.
    #[test]
    fn o_wav_gerado_tem_o_formato_que_o_whisper_exige() {
        let bytes = escrever_wav(&[0, 1, -1, 2]).expect("escreve");
        let leitor = hound::WavReader::new(Cursor::new(bytes)).expect("lê de volta");

        assert_eq!(leitor.spec().sample_rate, TAXA_DO_WHISPER);
        assert_eq!(leitor.spec().channels, 1);
        assert_eq!(leitor.spec().bits_per_sample, 16);
    }
}
