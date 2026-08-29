export { call, isTauriRuntime, TauriCommandError } from './client'
export {
  deviceState,
  discoverDevices,
  importTuyaDevices,
  irKeys,
  knownDevices,
  sensorStates,
  setDeviceDp,
  setDeviceHidden,
  setDevicePower,
  sendIrKey,
  setLight,
} from './casa'
export {
  browserBounds,
  browserClose,
  browserExternal,
  browserHistory,
  browserNavigate,
  browserOpen,
  browserSearch,
  browserSelect,
  browserState,
} from './navegador'
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
  escolherClipeDeVoz,
  listVoices,
  speakText,
  uploadVoiceReference,
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
