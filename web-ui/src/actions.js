export const CLEAR_MEDIA = "CLEAR_MEDIA";
export const SET_CONFIG = "SET_CONFIG";
export const SET_ELAPSED = "SET_ELAPSED";
export const SET_MEDIA = "SET_MEDIA";
export const SET_PLAYBACK = "SET_PLAYBACK";
export const SET_PLAYLIST = "SET_PLAYLIST";
export const TOGGLE_PLAYBACK = "TOGGLE_PLAYBACK";

export function setConfig(duration) {
  return { type: SET_ELAPSED, duration };
}

export function setElapsed(elapsed) {
  return { type: SET_ELAPSED, elapsed };
}

export function setMedia(media, elapsed) {
  return { type: SET_MEDIA, media, elapsed };
}

export function setPlayback(isPlaying) {
  return { type: SET_PLAYBACK, isPlaying };
}

export function setPlaylist(name) {
  return { type: SET_PLAYLIST, name };
}

export function togglePlayback() {
  return { type: TOGGLE_PLAYBACK };
}
