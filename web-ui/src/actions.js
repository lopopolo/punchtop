export const SET_MEDIA = "SET_MEDIA";
export const TOGGLE_PLAYBACK = "TOGGLE_PLAYBACK";
export const SET_ELAPSED = "SET_ELAPSED";

export function setMedia(media) {
  return { type: SET_MEDIA, media };
}

export function togglePlayback() {
  return { type: TOGGLE_PLAYBACK };
}

export function setElapsed(elapsed) {
  return { type: SET_ELAPSED, elapsed };
}
