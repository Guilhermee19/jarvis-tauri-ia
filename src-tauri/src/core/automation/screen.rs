//! Captura de tela via `xcap`.

use serde::Serialize;
use xcap::Monitor;

use super::{AutomationError, CapturedImage};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

pub fn list_monitors() -> Result<Vec<MonitorInfo>, AutomationError> {
    let monitors = Monitor::all().map_err(screen)?;

    monitors.iter().map(describe).collect()
}

/// `monitor_id` vem de [`list_monitors`]. `None` cai no monitor principal, que é o
/// que o agente (v0.2+) vai querer quando pedir "uma foto da tela" sem qualificar.
pub fn capture_screen(monitor_id: Option<u32>) -> Result<CapturedImage, AutomationError> {
    let monitors = Monitor::all().map_err(screen)?;
    if monitors.is_empty() {
        return Err(AutomationError::NoMonitor);
    }

    let monitor = match monitor_id {
        Some(id) => find(&monitors, id)?,
        None => primary(&monitors)?,
    };

    let image = monitor.capture_image().map_err(screen)?;
    CapturedImage::from_screen(image.width(), image.height(), image.into_raw())
}

fn find(monitors: &[Monitor], id: u32) -> Result<&Monitor, AutomationError> {
    monitors
        .iter()
        .find(|monitor| monitor.id().is_ok_and(|current| current == id))
        .ok_or(AutomationError::UnknownMonitor(id))
}

/// Sem monitor marcado como principal (acontece em algumas configurações de
/// múltiplas telas), o primeiro da lista é melhor do que falhar.
fn primary(monitors: &[Monitor]) -> Result<&Monitor, AutomationError> {
    let primary = monitors
        .iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false));

    primary
        .or_else(|| monitors.first())
        .ok_or(AutomationError::NoMonitor)
}

fn describe(monitor: &Monitor) -> Result<MonitorInfo, AutomationError> {
    Ok(MonitorInfo {
        id: monitor.id().map_err(screen)?,
        name: monitor.name().map_err(screen)?,
        width: monitor.width().map_err(screen)?,
        height: monitor.height().map_err(screen)?,
        is_primary: monitor.is_primary().map_err(screen)?,
    })
}

fn screen(error: xcap::XCapError) -> AutomationError {
    AutomationError::Screen(error.to_string())
}
