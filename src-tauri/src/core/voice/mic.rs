//! Captura do microfone via `cpal`.
//!
//! O `Stream` do cpal não é `Send`: ele precisa nascer, viver e morrer na mesma
//! thread. Por isso a gravação é uma thread dedicada que é dona do stream, e o
//! resto do app só enxerga os `Arc` compartilhados com ela.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat};
use serde::Serialize;

use super::VoiceError;
use crate::core::lock;

/// De quanto em quanto tempo o medidor de volume é publicado para a UI. 50 ms
/// (20 Hz) é o suficiente para a barra parecer contínua sem inundar o IPC.
const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

/// O que sobra de uma gravação: um WAV em disco e os números que a transcrição
/// (v0.2+) vai querer saber antes de abrir o arquivo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub path: String,
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub sample_count: usize,
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    worker: JoinHandle<()>,
}

impl Recorder {
    /// `on_level` é chamado a cada [`LEVEL_INTERVAL`] com o pico do intervalo
    /// (0.0–1.0). Recebe um callback em vez de emitir evento do Tauri porque
    /// `core` não conhece o Tauri — quem traduz isso em evento é `commands`.
    pub fn start<F>(on_level: F) -> Result<Self, VoiceError>
    where
        F: Fn(f32) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let peak = Arc::new(AtomicU32::new(0));
        let (ready_tx, ready_rx) = mpsc::channel();

        let worker = thread::spawn({
            let stop = Arc::clone(&stop);
            let samples = Arc::clone(&samples);
            let peak = Arc::clone(&peak);

            move || {
                // O stream fica vivo dentro deste escopo e morre junto com a
                // thread — é o `drop` dele que solta o dispositivo.
                let stream = match open_stream(samples, Arc::clone(&peak)) {
                    Ok(opened) => {
                        let _ = ready_tx.send(Ok(opened.sample_rate));
                        opened.stream
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                while !stop.load(Ordering::Relaxed) {
                    thread::sleep(LEVEL_INTERVAL);
                    on_level(f32::from_bits(peak.swap(0, Ordering::Relaxed)));
                }

                drop(stream);
            }
        });

        // Uma thread que morreu antes de responder só pode ter entrado em pânico.
        let sample_rate = ready_rx.recv().map_err(|_| {
            VoiceError::Microphone("a thread de captura morreu ao iniciar".into())
        })??;

        Ok(Self {
            stop,
            samples,
            sample_rate,
            worker,
        })
    }

    /// Consome o recorder: gravar duas vezes exige começar do zero, o que evita um
    /// estado "parado mas ainda com o dispositivo aberto".
    pub fn stop(self, path: &Path) -> Result<Recording, VoiceError> {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.worker.join();

        let samples = std::mem::take(&mut *lock(&self.samples));
        write_wav(path, &samples, self.sample_rate)?;

        Ok(Recording {
            path: path.to_string_lossy().into_owned(),
            duration_seconds: samples.len() as f32 / self.sample_rate as f32,
            sample_rate: self.sample_rate,
            sample_count: samples.len(),
        })
    }
}

struct OpenStream {
    stream: cpal::Stream,
    sample_rate: u32,
}

fn open_stream(
    samples: Arc<Mutex<Vec<f32>>>,
    peak: Arc<AtomicU32>,
) -> Result<OpenStream, VoiceError> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or(VoiceError::NoInputDevice)?;

    let supported = device.default_input_config().map_err(classify)?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let config = supported.config();

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, samples, peak, channels),
        SampleFormat::I16 => build_stream::<i16>(&device, &config, samples, peak, channels),
        SampleFormat::U16 => build_stream::<u16>(&device, &config, samples, peak, channels),
        other => {
            return Err(VoiceError::Microphone(format!(
                "formato de amostra não suportado pelo Jarvis: {other:?}"
            )))
        }
    }
    .map_err(classify)?;

    stream.play().map_err(classify)?;

    Ok(OpenStream {
        stream,
        sample_rate,
    })
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    peak: Arc<AtomicU32>,
    channels: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Só o primeiro canal: para fala, mixar estéreo não acrescenta nada e a
            // transcrição quer mono de qualquer jeito.
            let mono = data
                .chunks(channels)
                .filter_map(|frame| frame.first().map(|&sample| f32::from_sample_(sample)));

            let mut buffer = lock(&samples);
            for sample in mono {
                store_peak(&peak, sample.abs());
                buffer.push(sample);
            }
        },
        |error| eprintln!("[jarvis] erro no stream do microfone: {error}"),
        None,
    )
}

/// O pico viaja como os bits de um `f32` dentro de um `AtomicU32` para não precisar
/// de mutex no callback de áudio (que não pode bloquear). Para valores não negativos
/// a ordem dos bits é a mesma dos floats, então `fetch_max` funciona como máximo.
fn store_peak(peak: &AtomicU32, value: f32) {
    peak.fetch_max(value.to_bits(), Ordering::Relaxed);
}

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), VoiceError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| VoiceError::WavWrite(error.to_string()))?;
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|error| VoiceError::WavWrite(error.to_string()))?;

    for &sample in samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        writer
            .write_sample(pcm)
            .map_err(|error| VoiceError::WavWrite(error.to_string()))?;
    }

    writer
        .finalize()
        .map_err(|error| VoiceError::WavWrite(error.to_string()))
}

/// O cpal não distingue "sem permissão" de outros erros de dispositivo. No Windows,
/// negar o microfone no painel de privacidade chega como erro de backend com
/// `0x80070005` / "access is denied". A heurística é feia, mas a alternativa é o
/// usuário ler "falha ao abrir o microfone" e não descobrir que é permissão.
fn classify(error: impl std::fmt::Display) -> VoiceError {
    let message = error.to_string();
    let lowered = message.to_lowercase();

    if lowered.contains("denied") || lowered.contains("0x80070005") {
        VoiceError::MicrophoneDenied
    } else {
        VoiceError::Microphone(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_pico_sobrevive_a_ida_e_volta_pelos_bits_do_atomico() {
        let peak = AtomicU32::new(0);

        store_peak(&peak, 0.25);
        store_peak(&peak, 0.75);
        store_peak(&peak, 0.5);

        assert!((f32::from_bits(peak.swap(0, Ordering::Relaxed)) - 0.75).abs() < f32::EPSILON);
        // O `swap` zera: o próximo intervalo do medidor começa limpo.
        assert_eq!(peak.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn grava_wav_de_16_bits_com_a_duracao_esperada() {
        let dir = std::env::temp_dir().join("jarvis-testes");
        let path = dir.join("mic.wav");
        let samples = vec![0.0_f32; 16_000];

        write_wav(&path, &samples, 16_000).expect("grava o wav");

        let reader = hound::WavReader::open(&path).expect("lê o wav");
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.duration(), 16_000);
    }
}
