export { call, isTauriRuntime, TauriCommandError } from './client'
export { clearHistory, getHistory, sendMessage } from './chat'
export { getSettings, saveSettings } from './settings'
export {
  hideWindow,
  identifyTrack,
  isWindowMaximized,
  minimizeWindow,
  nowPlaying,
  pressMediaKey,
  quitApp,
  showWindow,
  toggleMaximizeWindow,
  toggleWindow,
  type MediaKey,
  type NowPlaying,
} from './system'
export { JarvisEvent, onJarvisEvent, type Faixa, type UiAction } from './events'
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
