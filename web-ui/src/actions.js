export const SET_ACTIVE_TRACK = "SET_ACTIVE_TRACK";
export const SET_ELAPSED = "SET_ELAPSED";
export const SET_MEDIA = "SET_MEDIA";
export const SET_PLAYLIST = "SET_PLAYLIST";
export const TOGGLE_PLAYBACK = "TOGGLE_PLAYBACK";

export function setActiveTrack(id) {
  return { type: SET_ACTIVE_TRACK, id };
}

export function setElapsed(elapsed) {
  return { type: SET_ELAPSED, elapsed };
}

export function setMedia(media) {
  return { type: SET_MEDIA, media };
}

export function setPlaylist(name, initial) {
  return { type: SET_PLAYLIST, name, initial };
}

export function togglePlayback() {
  return { type: TOGGLE_PLAYBACK };
}
