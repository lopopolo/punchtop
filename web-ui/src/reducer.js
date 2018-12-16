import clamp from "clamp";
import { combineReducers } from "redux";
import { connectRouter } from "connected-react-router";

import {
  CLEAR_MEDIA,
  SET_CONFIG,
  SET_ELAPSED,
  SET_MEDIA,
  SET_PLAYLIST,
  TOGGLE_PLAYBACK
} from "./actions";

const reducer = (state = {}, action) => {
  switch (action.type) {
    case CLEAR_MEDIA: {
      const media = Object.assign({}, state.media, {
        current: null
      });
      return Object.assign({}, state, { media });
    }
    case SET_CONFIG: {
      const config = Object.assign({}, state.config, {
        duration: action.duration
      });
      return Object.assign({}, state, { config });
    }
    case SET_MEDIA: {
      const media = Object.assign({}, state.media, {
        current: action.media
      });
      return Object.assign({}, state, { media });
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

const rootReducer = history =>
  combineReducers({
    punchtop: reducer,
    router: connectRouter(history)
  });

export default rootReducer;
