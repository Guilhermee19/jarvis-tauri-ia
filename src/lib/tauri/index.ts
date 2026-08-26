export { call, isTauriRuntime, TauriCommandError } from './client'
export { clearHistory, getHistory, sendMessage } from './chat'
export { getSettings, saveSettings } from './settings'
export {
  hideWindow,
  isWindowMaximized,
  minimizeWindow,
  quitApp,
  showWindow,
  toggleMaximizeWindow,
  toggleWindow,
} from './system'
export { JarvisEvent, onJarvisEvent } from './events'
export {
  isRecording,
  listVoices,
  speakText,
  startRecording,
  stopRecording,
  transcribe,
} from './voice'
export {
  captureScreenshot,
  captureWebcamFrame,
  closeWebcam,
  isWebcamOpen,
  listMonitors,
  openWebcam,
} from './automation'
