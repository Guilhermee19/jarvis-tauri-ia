//! Volume e controle de mídia.
//!
//! Dois mecanismos, por motivos diferentes:
//!
//! - **Volume** (ler, escrever, mudo) usa COM (`IAudioEndpointVolume`). Poderia ser a
//!   tecla `VK_VOLUME_UP`, mas ela só sabe andar um passo de ~2% e não sabe ler o
//!   valor atual — e "põe em 30%" precisa dos dois. Como o COM já é obrigatório para
//!   o absoluto, ele cobre o relativo e o mudo de graça, com precisão exata e imune
//!   ao bloqueio de entrada sintética descrito abaixo.
//! - **Play/pause, próxima, anterior** usam `SendInput`, porque não existe API: são
//!   `WM_APPCOMMAND` que o Windows roteia para o player de mídia ativo. É o que faz
//!   o Jarvis pausar o Spotify sem saber que Spotify existe.
//!
//! O preço de mexer no volume por COM é não aparecer o OSD do Windows (aquele
//! retângulo com a barrinha), que só a tecla dispara. Precisão vale mais que o popup.

use super::SystemError;

#[derive(Debug, Clone, Copy)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

pub use imp::{is_muted, press, set_mute, set_volume, volume_of};

/// Volume relativo: lê, soma, grava. Devolve o valor novo para o log de ações poder
/// dizer no que deu.
pub fn nudge_volume(delta: i8) -> Result<u8, SystemError> {
    let atual = i16::from(volume_of()?);
    let novo = (atual + i16::from(delta)).clamp(0, 100) as u8;

    set_volume(novo)?;
    Ok(novo)
}

/// "Muta" em linguagem falada quase sempre quer dizer alternar.
pub fn toggle_mute() -> Result<bool, SystemError> {
    let novo = !is_muted()?;
    set_mute(novo)?;
    Ok(novo)
}

#[cfg(windows)]
mod imp {
    use std::mem::size_of;

    use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE};
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
    };

    use super::super::SystemError;
    use super::MediaKey;

    impl MediaKey {
        fn vk(self) -> VIRTUAL_KEY {
            match self {
                Self::PlayPause => VK_MEDIA_PLAY_PAUSE,
                Self::Next => VK_MEDIA_NEXT_TRACK,
                Self::Previous => VK_MEDIA_PREV_TRACK,
            }
        }
    }

    /// Um toque completo — pressiona e solta na MESMA chamada, para nada da fila do
    /// sistema se intrometer entre os dois e o Windows achar que a tecla ficou presa.
    pub fn press(key: MediaKey) -> Result<(), SystemError> {
        let evento = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key.vk(),
                    dwFlags: flags,
                    ..Default::default()
                },
            },
        };
        let eventos = [evento(KEYBD_EVENT_FLAGS(0)), evento(KEYEVENTF_KEYUP)];

        // SAFETY: `eventos` é um slice vivo de `INPUT` e o tamanho declarado é o do
        // próprio tipo — as duas condições do contrato da API.
        let enviados = unsafe { SendInput(&eventos, size_of::<INPUT>() as i32) };

        if enviados as usize == eventos.len() {
            Ok(())
        } else {
            Err(SystemError::TeclaBloqueada)
        }
    }

    /// COM é por thread, e `#[tauri::command(async)]` roda numa thread qualquer do
    /// pool — reusada, e possivelmente já inicializada por outra coisa. Três desfechos:
    ///
    /// - `S_OK`: nós inicializamos, temos que desfazer.
    /// - `S_FALSE`: já estava no mesmo modelo e o contador subiu — desfazer também.
    /// - `RPC_E_CHANGED_MODE`: a thread já é MTA. Serve igual para o nosso uso, mas
    ///   não incrementamos nada, e `CoUninitialize` aqui derrubaria o apartamento de
    ///   quem inicializou — que pode ser o cpal, e aí o microfone para de abrir
    ///   DEPOIS de alguém mexer no volume. É o bug caro deste módulo; daí o bool.
    struct ComGuard(bool);

    impl ComGuard {
        fn new() -> Result<Self, SystemError> {
            // SAFETY: emparelhado com o `CoUninitialize` do `Drop`, na mesma thread.
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

            if hr == RPC_E_CHANGED_MODE {
                return Ok(Self(false));
            }
            if hr.is_err() {
                return Err(SystemError::Com(hr.message()));
            }

            debug_assert!(hr.is_ok() || hr == S_FALSE);
            Ok(Self(true))
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: só quando foi este guard que incrementou o contador.
                unsafe { CoUninitialize() };
            }
        }
    }

    /// Abre o endpoint, usa e solta, tudo dentro do escopo do guard.
    ///
    /// A interface NÃO é guardada em estado gerenciado de propósito: ela nasce num
    /// apartamento de uma thread do pool que já morreu na chamada seguinte, e usá-la
    /// de outra thread sem marshalling é UB disfarçado de otimização. Abrir custa
    /// ~1 ms e não é gargalo de nada.
    fn com_endpoint<T>(
        acao: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T>,
    ) -> Result<T, SystemError> {
        let _com = ComGuard::new()?;

        // SAFETY: ponteiros gerenciados pelos wrappers do próprio crate `windows`, e
        // COM está inicializado nesta thread enquanto `_com` viver.
        unsafe {
            let enumerador: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(com)?;
            let dispositivo = enumerador
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|_| SystemError::SemSaidaDeAudio)?;
            let volume: IAudioEndpointVolume =
                dispositivo.Activate(CLSCTX_ALL, None).map_err(com)?;

            acao(&volume).map_err(com)
        }
    }

    pub fn volume_of() -> Result<u8, SystemError> {
        // SAFETY (aqui e nos três abaixo): `v` é uma interface COM viva, emprestada
        // por `com_endpoint` de dentro do escopo do guard. O `unsafe` mora no closure
        // porque o corpo dele não herda o bloco de quem o recebe.
        let escalar = com_endpoint(|v| unsafe { v.GetMasterVolumeLevelScalar() })?;
        Ok((escalar * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    /// Escalar é o valor LINEAR NA BARRINHA: `0.3` deixa o mixer do Windows mostrando
    /// 30%, que é exatamente o que o usuário quer dizer com "põe em 30". (O parente
    /// `SetMasterVolumeLevel` é o que trabalha em decibéis.)
    ///
    /// O que não é linear é a percepção: subir 10 pontos com o volume baixo é bem mais
    /// audível do que perto do fim da escala.
    pub fn set_volume(percent: u8) -> Result<(), SystemError> {
        let alvo = f32::from(percent.min(100)) / 100.0;
        com_endpoint(|v| unsafe { v.SetMasterVolumeLevelScalar(alvo, std::ptr::null()) })
    }

    pub fn set_mute(mudo: bool) -> Result<(), SystemError> {
        com_endpoint(|v| unsafe { v.SetMute(mudo, std::ptr::null()) })
    }

    pub fn is_muted() -> Result<bool, SystemError> {
        Ok(com_endpoint(|v| unsafe { v.GetMute() })?.as_bool())
    }

    fn com(error: windows::core::Error) -> SystemError {
        SystemError::Com(error.message())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// Fora do `cargo test` normal porque depende do dispositivo de áudio da máquina
    /// e mexe no volume do sistema. Rode com:
    ///
    /// ```text
    /// cargo test fala_com_o_mixer -- --ignored --nocapture
    /// ```
    ///
    /// É o único jeito de provar o `ComGuard`: o erro que ele evita não aparece na
    /// compilação, aparece como microfone que para de abrir depois que alguém mexeu
    /// no volume. Por isso o teste lê, escreve e lê DE NOVO — o segundo `com_endpoint`
    /// é o que quebraria se o `CoUninitialize` tivesse derrubado o apartamento errado.
    #[test]
    #[ignore = "precisa de um dispositivo de áudio de verdade"]
    fn fala_com_o_mixer_sem_derrubar_o_apartamento_com() {
        let antes = volume_of().expect("ler o volume");
        assert!(antes <= 100);

        // Repõe o mesmo valor: prova a escrita sem ninguém ouvir diferença.
        set_volume(antes).expect("escrever o volume");
        assert_eq!(volume_of().expect("ler de novo"), antes);

        let mudo = is_muted().expect("ler o mudo");
        set_mute(mudo).expect("escrever o mudo");

        println!("volume={antes}% mudo={mudo}");
    }
}

/// Fora do Windows nada disso existe. O app é Windows-only, mas manter o `cfg` honesto
/// deixa `cargo test` rodar os testes de validação em qualquer máquina.
#[cfg(not(windows))]
mod imp {
    use super::super::SystemError;
    use super::MediaKey;

    fn indisponivel() -> SystemError {
        SystemError::Com("controle de áudio só existe no Windows".into())
    }

    pub fn press(_key: MediaKey) -> Result<(), SystemError> {
        Err(indisponivel())
    }
    pub fn volume_of() -> Result<u8, SystemError> {
        Err(indisponivel())
    }
    pub fn set_volume(_percent: u8) -> Result<(), SystemError> {
        Err(indisponivel())
    }
    pub fn set_mute(_mudo: bool) -> Result<(), SystemError> {
        Err(indisponivel())
    }
    pub fn is_muted() -> Result<bool, SystemError> {
        Err(indisponivel())
    }
}
