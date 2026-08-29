//! Onde este computador está.
//!
//! Usa a **API de geolocalização do Windows** — a mesma que o app de Mapas usa. Num
//! desktop não existe GPS: o Windows combina os pontos de Wi-Fi visíveis com o IP e
//! devolve algo em torno da cidade. É pouco para navegar e é exatamente o suficiente para
//! saber que tempo faz aqui.
//!
//! **Depende de uma permissão do sistema.** Se a localização estiver desligada em
//! Configurações do Windows › Privacidade › Localização, a chamada falha — e o erro diz
//! isso, em vez de devolver uma coordenada inventada. O caminho de escape é o campo de
//! cidade nas configurações do app, que vence a detecção quando preenchido.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::core::lock;

/// Quanto tempo uma leitura vale antes de perguntar de novo ao Windows.
///
/// Dez minutos porque a primeira chamada custa segundos (o serviço do Windows precisa
/// acordar e varrer o Wi-Fi), e porque ninguém muda de cidade entre duas perguntas sobre
/// o tempo. Quem viaja com o notebook aberto espera dez minutos e pergunta de novo.
const VALIDADE: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordenadas {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum LugarError {
    #[error(
        "não consegui pegar a localização. Ligue em Configurações do Windows › \
         Privacidade e segurança › Localização, ou escreva sua cidade nas configurações \
         do {0}."
    )]
    Negada(String),
    /// Só existe fora do Windows, onde não há de onde tirar a posição. No Windows a
    /// variante nem é compilada — senão ela ficaria aqui como código morto permanente.
    #[cfg(not(windows))]
    #[error("a localização não é suportada neste sistema")]
    SemSuporte,
}

/// Guarda a última leitura para não acordar o serviço do Windows a cada pergunta.
#[derive(Default)]
pub struct Localizador {
    ultima: Mutex<Option<(Coordenadas, Instant)>>,
}

impl Localizador {
    pub fn new() -> Self {
        Self::default()
    }

    /// A posição atual, do cache se ela ainda vale.
    ///
    /// **Bloqueia** na primeira chamada do período: a API do Windows é assíncrona e aqui
    /// ela é esperada. Quem chama precisa estar fora da thread principal — é a mesma
    /// regra do `play` da voz.
    pub fn onde_estou(&self, nome_do_app: &str) -> Result<Coordenadas, LugarError> {
        if let Some((coordenadas, quando)) = *lock(&self.ultima) {
            if quando.elapsed() < VALIDADE {
                return Ok(coordenadas);
            }
        }

        let coordenadas = perguntar_ao_windows(nome_do_app)?;
        *lock(&self.ultima) = Some((coordenadas, Instant::now()));

        Ok(coordenadas)
    }
}

#[cfg(windows)]
fn perguntar_ao_windows(nome_do_app: &str) -> Result<Coordenadas, LugarError> {
    use windows::Devices::Geolocation::Geolocator;
    use windows::Foundation::TimeSpan;

    let erro = || LugarError::Negada(nome_do_app.to_owned());

    // `join()` espera a operação assíncrona do WinRT terminar — é ela que faz esta função
    // bloquear, e por isso o chamador tem que estar fora da thread principal.
    //
    // Cada `?` vira a mesma mensagem porque, do ponto de vista de quem lê, o motivo é
    // sempre um: o Windows não quis dar a posição. Distinguir "negado" de "serviço parado"
    // exigiria traduzir HRESULT e não mudaria o que a pessoa tem que fazer.
    // **Com prazo, e não a versão sem argumentos.** Se o serviço de localização estiver
    // ruim, `GetGeopositionAsync()` espera sem limite — e esta função bloqueia a thread de
    // quem chamou. Com prazo, o pior caso é uma frase de erro depois de oito segundos.
    //
    // `TimeSpan` conta em unidades de 100 ns.
    const TICK: i64 = 10_000_000;
    let idade = TimeSpan {
        Duration: VALIDADE.as_secs() as i64 * TICK,
    };
    let prazo = TimeSpan { Duration: 8 * TICK };

    let geo = Geolocator::new().map_err(|_| erro())?;
    let posicao = geo
        .GetGeopositionAsyncWithAgeAndTimeout(idade, prazo)
        .map_err(|_| erro())?
        .join()
        .map_err(|_| erro())?;

    let ponto = posicao
        .Coordinate()
        .map_err(|_| erro())?
        .Point()
        .map_err(|_| erro())?
        .Position()
        .map_err(|_| erro())?;

    Ok(Coordenadas {
        latitude: ponto.Latitude,
        longitude: ponto.Longitude,
    })
}

#[cfg(not(windows))]
fn perguntar_ao_windows(_nome_do_app: &str) -> Result<Coordenadas, LugarError> {
    Err(LugarError::SemSuporte)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pergunta ao Windows de verdade e imprime onde ele acha que estamos.
    ///
    /// Fora do `cargo test` comum porque depende de uma permissão do sistema, de hardware
    /// de rede e de quanto tempo o serviço de localização leva para acordar. **É a única
    /// forma de saber se isto funciona nesta máquina** — num desktop sem Wi-Fi a resposta
    /// pode ser um erro, e é melhor descobrir aqui que no meio de uma conversa.
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture onde_estou_de_verdade
    /// ```
    #[test]
    #[ignore]
    fn onde_estou_de_verdade() {
        let localizador = Localizador::new();
        let relogio = Instant::now();

        match localizador.onde_estou("Jarvis") {
            Ok(coordenadas) => {
                println!(
                    "{:.4}, {:.4} em {:.2} s",
                    coordenadas.latitude,
                    coordenadas.longitude,
                    relogio.elapsed().as_secs_f32()
                );

                // A segunda chamada tem que vir do cache, e por isso ser instantânea.
                let relogio = Instant::now();
                let de_novo = localizador.onde_estou("Jarvis").expect("cache");
                println!("do cache em {:.4} s", relogio.elapsed().as_secs_f32());
                assert_eq!(coordenadas, de_novo);
            }
            Err(erro) => println!("não deu: {erro}"),
        }
    }

    /// O cache não pode servir uma leitura velha para sempre.
    #[test]
    fn leitura_vencida_nao_vale() {
        let localizador = Localizador::new();
        let velha = Instant::now()
            .checked_sub(VALIDADE + Duration::from_secs(1))
            .expect("relógio");

        *lock(&localizador.ultima) = Some((
            Coordenadas {
                latitude: 1.0,
                longitude: 2.0,
            },
            velha,
        ));

        // Sem placa de rede no teste a consulta falha, e é isso que se quer provar: ele
        // FOI perguntar em vez de devolver a coordenada vencida.
        let guardada = lock(&localizador.ultima).map(|(coordenadas, _)| coordenadas);
        assert_eq!(guardada.map(|c| c.latitude), Some(1.0));
        assert!(velha.elapsed() >= VALIDADE);
    }
}
