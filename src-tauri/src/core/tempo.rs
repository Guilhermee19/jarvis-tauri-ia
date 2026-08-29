//! Previsão do tempo, pela Open-Meteo.
//!
//! **Sem chave de API.** É a mesma razão de a busca usar Wikipedia por padrão: uma feature
//! que exige cadastro é uma feature que a maioria nunca liga. A Open-Meteo é gratuita para
//! uso não comercial e não pede nada.
//!
//! São duas rotas, e elas moram em domínios diferentes:
//!
//! - `geocoding-api.open-meteo.com/v1/search` — nome de lugar vira coordenada
//! - `api.open-meteo.com/v1/forecast` — coordenada vira previsão
//!
//! Quando a pergunta é sobre "aqui", a coordenada vem do [`crate::core::lugar`] e a
//! primeira rota nem é chamada.

use std::time::Duration;

use serde::Deserialize;

use crate::core::lugar::Coordenadas;

const BUSCA: &str = "https://geocoding-api.open-meteo.com/v1/search";
const PREVISAO: &str = "https://api.open-meteo.com/v1/forecast";

/// Curto de propósito: isto entra na espera de uma conversa. Melhor "não consegui ver o
/// tempo agora" em cinco segundos do que uma resposta certa em trinta.
const TIMEOUT: Duration = Duration::from_secs(8);

/// Quantos dias pedir. Três cobre "e amanhã?" sem encher a resposta falada.
const DIAS: u8 = 3;

#[derive(Debug, thiserror::Error)]
pub enum TempoError {
    #[error("não achei nenhum lugar chamado \"{0}\"")]
    LugarDesconhecido(String),
    #[error("não consegui falar com o serviço de previsão: {0}")]
    Rede(String),
    #[error("o serviço de previsão recusou a consulta (HTTP {0})")]
    Recusada(u16),
}

/// Um lugar com nome, para a resposta poder dizer de onde está falando.
#[derive(Debug, Clone)]
pub struct Lugar {
    pub nome: String,
    /// Estado/província e país, já juntos. **Não é enfeite**: "Lisboa" existe em Portugal
    /// e em Moçambique, "Belo Horizonte" no Brasil e em Angola. Sem isto, a resposta erra
    /// de continente sem dar nenhum sinal de que errou.
    pub regiao: String,
    pub coordenadas: Coordenadas,
}

impl Lugar {
    /// "Teresópolis, Rio de Janeiro, Brasil" — ou só o nome, quando não veio região.
    pub fn completo(&self) -> String {
        if self.regiao.is_empty() {
            self.nome.clone()
        } else {
            format!("{}, {}", self.nome, self.regiao)
        }
    }
}

/// Acha o primeiro lugar com esse nome.
///
/// O primeiro, e não uma lista para escolher: a Open-Meteo ordena por população, e quem
/// pergunta "como está o tempo em Lisboa" quer a capital de Portugal, não a vila
/// homônima em Moçambique. A desambiguação fica na RESPOSTA, que diz o estado e o país.
pub async fn procurar(http: &reqwest::Client, nome: &str) -> Result<Lugar, TempoError> {
    let resposta = http
        .get(BUSCA)
        .query(&[
            ("name", nome),
            ("count", "1"),
            ("language", "pt"),
            ("format", "json"),
        ])
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(rede)?;

    let achados: Busca = conferir(resposta).await?.json().await.map_err(rede)?;

    achados
        .results
        .into_iter()
        .next()
        .map(|achado| Lugar {
            regiao: [achado.admin1.unwrap_or_default(), achado.country]
                .iter()
                .filter(|parte| !parte.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            nome: achado.name,
            coordenadas: Coordenadas {
                latitude: achado.latitude,
                longitude: achado.longitude,
            },
        })
        .ok_or_else(|| TempoError::LugarDesconhecido(nome.to_owned()))
}

/// A previsão para uma coordenada.
pub async fn consultar(
    http: &reqwest::Client,
    onde: Coordenadas,
) -> Result<Previsao, TempoError> {
    let resposta = http
        .get(PREVISAO)
        .query(&[
            ("latitude", onde.latitude.to_string().as_str()),
            ("longitude", onde.longitude.to_string().as_str()),
            ("current", "temperature_2m,relative_humidity_2m,weather_code"),
            (
                "daily",
                "temperature_2m_min,temperature_2m_max,weather_code,precipitation_probability_max",
            ),
            // `auto` deixa o serviço resolver o fuso pela coordenada. Sem isto, "hoje"
            // seria o dia em UTC, e a previsão de hoje viraria a de ontem à noite.
            ("timezone", "auto"),
            ("forecast_days", DIAS.to_string().as_str()),
        ])
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(rede)?;

    let bruta: Resposta = conferir(resposta).await?.json().await.map_err(rede)?;

    Ok(Previsao {
        temperatura: bruta.current.temperature_2m,
        umidade: bruta.current.relative_humidity_2m,
        ceu: bruta.current.weather_code,
        dias: (0..bruta.daily.time.len().min(DIAS as usize))
            .map(|i| Dia {
                minima: bruta.daily.temperature_2m_min[i],
                maxima: bruta.daily.temperature_2m_max[i],
                ceu: bruta.daily.weather_code[i],
                chuva: bruta.daily.precipitation_probability_max[i].unwrap_or(0),
            })
            .collect(),
    })
}

#[derive(Debug, Clone)]
pub struct Previsao {
    pub temperatura: f32,
    pub umidade: u8,
    pub ceu: u8,
    pub dias: Vec<Dia>,
}

#[derive(Debug, Clone, Copy)]
pub struct Dia {
    pub minima: f32,
    pub maxima: f32,
    pub ceu: u8,
    pub chuva: u8,
}

impl Previsao {
    /// A previsão em uma frase, pronta para ser dita em voz alta.
    ///
    /// Frase e não tabela porque o destino principal é o TTS: número solto e travessão
    /// viram gagueira quando lidos. Temperatura arredondada pelo mesmo motivo — "vinte e
    /// sete vírgula cinco graus" não é como alguém fala.
    pub fn frase(&self, lugar: Option<&str>) -> String {
        // `None` é o caso de "aqui": a coordenada veio do Windows e não tem nome, porque
        // a Open-Meteo não faz geocodificação reversa. Dizer "Aqui" é honesto; inventar
        // um nome de cidade a partir da coordenada seria chute.
        let mut texto = match lugar {
            Some(lugar) => format!("Em {lugar}: "),
            None => "Aqui: ".to_owned(),
        };

        texto.push_str(&format!(
            "{:.0} graus, {}, umidade {}%.",
            self.temperatura,
            descricao(self.ceu),
            self.umidade
        ));

        for (quando, dia) in ["Hoje", "Amanhã", "Depois de amanhã"].iter().zip(&self.dias) {
            texto.push_str(&format!(
                " {quando}, entre {:.0} e {:.0} graus, {}",
                dia.minima,
                dia.maxima,
                descricao(dia.ceu)
            ));

            // Chance de chuva só entra quando é relevante: "0% de chance de chuva" em todo
            // dia de sol é ruído numa resposta falada.
            if dia.chuva >= 30 {
                texto.push_str(&format!(", {}% de chance de chuva", dia.chuva));
            }
            texto.push('.');
        }

        texto
    }
}

/// Código WMO em português.
///
/// A tabela é a da documentação da Open-Meteo, agrupada: os três níveis de chuva viram
/// "chuva fraca/moderada/forte", e o que não está previsto cai num genérico em vez de num
/// número que ninguém sabe ler.
fn descricao(codigo: u8) -> &'static str {
    match codigo {
        0 => "céu limpo",
        1 => "quase limpo",
        2 => "parcialmente nublado",
        3 => "nublado",
        45 | 48 => "com névoa",
        51 | 53 | 55 => "com garoa",
        56 | 57 => "com garoa congelante",
        61 | 80 => "com chuva fraca",
        63 | 81 => "com chuva moderada",
        65 | 82 => "com chuva forte",
        66 | 67 => "com chuva congelante",
        71 | 73 | 75 | 77 | 85 | 86 => "com neve",
        95 => "com trovoada",
        96 | 99 => "com trovoada e granizo",
        _ => "sem detalhe do céu",
    }
}

async fn conferir(resposta: reqwest::Response) -> Result<reqwest::Response, TempoError> {
    let status = resposta.status();

    if status.is_success() {
        Ok(resposta)
    } else {
        Err(TempoError::Recusada(status.as_u16()))
    }
}

fn rede(erro: reqwest::Error) -> TempoError {
    TempoError::Rede(erro.to_string())
}

// ---- o que a Open-Meteo devolve, só os campos que usamos ----

#[derive(Deserialize)]
struct Busca {
    /// Ausente — e não vazio — quando nada casa com o nome.
    #[serde(default)]
    results: Vec<Achado>,
}

#[derive(Deserialize)]
struct Achado {
    name: String,
    latitude: f64,
    longitude: f64,
    country: String,
    /// O estado/província. Nem todo lugar tem.
    #[serde(default)]
    admin1: Option<String>,
}

#[derive(Deserialize)]
struct Resposta {
    current: Agora,
    daily: Diario,
}

#[derive(Deserialize)]
struct Agora {
    temperature_2m: f32,
    relative_humidity_2m: u8,
    weather_code: u8,
}

#[derive(Deserialize)]
struct Diario {
    time: Vec<String>,
    temperature_2m_min: Vec<f32>,
    temperature_2m_max: Vec<f32>,
    weather_code: Vec<u8>,
    /// `null` em dias sem dado de precipitação, e não zero.
    precipitation_probability_max: Vec<Option<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frase_sai_pronta_para_falar() {
        let previsao = Previsao {
            temperatura: 27.5,
            umidade: 51,
            ceu: 0,
            dias: vec![
                Dia {
                    minima: 16.8,
                    maxima: 30.2,
                    ceu: 0,
                    chuva: 5,
                },
                Dia {
                    minima: 18.6,
                    maxima: 28.1,
                    ceu: 61,
                    chuva: 70,
                },
            ],
        };

        let frase = previsao.frase(Some("Teresópolis, Rio de Janeiro, Brasil"));
        assert!(previsao.frase(None).starts_with("Aqui: 28 graus"));

        assert!(frase.starts_with("Em Teresópolis, Rio de Janeiro, Brasil: 28 graus, céu limpo, umidade 51%."));
        // O dia de sol NÃO cita chance de chuva; o de chuva cita.
        assert!(frase.contains("Hoje, entre 17 e 30 graus, céu limpo."));
        assert!(frase.contains("Amanhã, entre 19 e 28 graus, com chuva fraca, 70% de chance de chuva."));
    }

    /// Um código que a tabela não conhece não pode virar número na frase falada.
    #[test]
    fn codigo_desconhecido_vira_texto() {
        assert_eq!(descricao(0), "céu limpo");
        assert_eq!(descricao(95), "com trovoada");
        assert_eq!(descricao(200), "sem detalhe do céu");
    }

    /// "Lisboa" existe em Portugal e em Moçambique. Sem a região na resposta, a pessoa não
    /// tem como saber que ele pegou a errada.
    #[test]
    fn o_lugar_se_apresenta_com_regiao_e_pais() {
        let lugar = Lugar {
            nome: "Lisboa".to_owned(),
            regiao: "Distrito de Lisboa, Portugal".to_owned(),
            coordenadas: Coordenadas {
                latitude: 38.7,
                longitude: -9.1,
            },
        };

        assert_eq!(lugar.completo(), "Lisboa, Distrito de Lisboa, Portugal");
    }

    /// Consulta a Open-Meteo de verdade, nas coordenadas que o Windows der.
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture tempo_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn tempo_de_verdade() {
        let bloco = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        let http = reqwest::Client::new();

        bloco.block_on(async {
            let aqui = crate::core::lugar::Localizador::new().onde_estou("Jarvis");

            match aqui {
                Ok(coordenadas) => match consultar(&http, coordenadas).await {
                    Ok(previsao) => println!("{}", previsao.frase(None)),
                    Err(erro) => println!("não consultou: {erro}"),
                },
                Err(erro) => println!("sem localização: {erro}"),
            }

            match procurar(&http, "Lisboa").await {
                Ok(lugar) => {
                    println!("achou: {}", lugar.completo());
                    match consultar(&http, lugar.coordenadas).await {
                        Ok(previsao) => println!("{}", previsao.frase(Some(&lugar.completo()))),
                        Err(erro) => println!("não consultou: {erro}"),
                    }
                }
                Err(erro) => println!("não achou: {erro}"),
            }
        });
    }
}
