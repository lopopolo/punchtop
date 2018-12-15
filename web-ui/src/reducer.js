import clamp from "clamp";
import { combineReducers } from "redux";
import { connectRouter } from "connected-react-router";

import {
  TOGGLE_PLAYBACK,
  SET_ACTIVE_TRACK,
  SET_ELAPSED,
  SET_PLAYLIST
} from "./actions";

const reducer = (state = {}, action) => {
  switch (action.type) {
    case SET_ACTIVE_TRACK: {
      const player = Object.assign({}, state.player, {
        current: action.id
      });
      return Object.assign({}, state, { player });
    }
    case SET_PLAYLIST: {
      const player = Object.assign({}, state.player, {
        current: action.initial
      });
      const source = Object.assign({}, state.config.source, {
        name: action.name
      });
      const config = Object.assign({}, state.config, {
        source
      });
      return Object.assign({}, state, { config, player });
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
