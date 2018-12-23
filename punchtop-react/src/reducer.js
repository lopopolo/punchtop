import clamp from "clamp";
import { combineReducers } from "redux";

import {
  CLEAR_MEDIA,
  SET_ACTIVE_DEVICE,
  SET_CONFIG,
  SET_ELAPSED,
  SET_MEDIA,
  SET_PLAYBACK,
  SET_PLAYLIST,
  TOGGLE_PLAYBACK
} from "./actions";

const initialState = {
  media: {
    current: null
  },
  config: {
    duration: 60,
    source: null
  },
  player: {
    elapsed: 0.0,
    isPlaying: false
  },
  device: {
    active: {},
    all: [
      { kind: "cast", name: "TV" },
      { kind: "local", name: "quadbox" },
      { kind: "cast", name: "Kitchen Home" },
      { kind: "cast", name: "Soundbar" }
    ]
  }
};

const reducer = (state = initialState, action) => {
  switch (action.type) {
    case CLEAR_MEDIA: {
      const media = Object.assign({}, state.media, {
        current: null
      });
      const player = Object.assign({}, state.player, {
        elapsed: clamp(0, 0, state.config.duration)
      });
      return Object.assign({}, state, { media, player });
    }
    case SET_ACTIVE_DEVICE: {
      const config = Object.assign({}, state.device, {
        kind: action.kind,
        name: action.name
      });
      return Object.assign({}, state, { config });
    }
    case SET_CONFIG: {
      const config = Object.assign({}, state.config, {
        duration: action.duration
      });
      return Object.assign({}, state, { config });
    }
    case SET_MEDIA: {
      const player = Object.assign({}, state.player, {
        elapsed: clamp(action.elapsed || 0, 0, state.config.duration)
      });
      const media = Object.assign({}, state.media, {
        current: action.media
      });
      return Object.assign({}, state, { media, player });
    }
    case SET_PLAYBACK: {
      if (action.isPlaying === state.player.isPlaying) {
        return state;
      }
      const player = Object.assign({}, state.player, {
        isPlaying: action.isPlaying
      });
      global.external.invoke(player.isPlaying ? "play" : "pause");
      return Object.assign({}, state, { player });
    }
    case SET_PLAYLIST: {
      const source = Object.assign({}, state.config.source, {
        name: action.name
      });
      const config = Object.assign({}, state.config, {
        source
      });
      return Object.assign({}, state, { config });
    }
    case TOGGLE_PLAYBACK: {
      const player = Object.assign({}, state.player, {
        isPlaying: !state.player.isPlaying
      });
      global.external.invoke(player.isPlaying ? "play" : "pause");
      return Object.assign({}, state, { player });
    }
    case SET_ELAPSED: {
      const player = Object.assign({}, state.player, {
        elapsed: clamp(action.elapsed, 0, state.config.duration)
      });
      return Object.assign({}, state, { player });
    }
    default:
      return state;
  }
};

const rootReducer = () =>
  combineReducers({
    punchtop: reducer
  });

export default rootReducer;
