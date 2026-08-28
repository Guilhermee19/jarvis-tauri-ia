export { call, isTauriRuntime, TauriCommandError } from './client'
export {
  deviceState,
  discoverDevices,
  importTuyaDevices,
  irKeys,
  knownDevices,
  setDeviceDp,
  setDeviceHidden,
  setDevicePower,
  sendIrKey,
  setLight,
} from './casa'
export { clearHistory, getHistory, sendMessage } from './chat'
export { getSettings, saveSettings } from './settings'
export {
  hideWindow,
  identifyTrack,
  isWindowMaximized,
  minimizeWindow,
  nowPlaying,
  performanceMetrics,
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
  listInputDevices,
  listVoices,
  speakText,
  startRecording,
  stopRecording,
  stopSpeaking,
  transcribe,
} from './voice'
export {
  captureScreenshot,
  captureWebcamFrame,
  closeWebcam,
  isWebcamOpen,
  listMonitors,
  listWebcamResolutions,
  openWebcam,
} from './automation'
