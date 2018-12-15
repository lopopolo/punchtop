import clamp from "clamp";
import { combineReducers } from "redux";
import { connectRouter } from "connected-react-router";

import { TOGGLE_PLAYBACK, SET_ELAPSED, SET_PLAYLIST } from "./actions";

const initialState = {
  media: {
    artist: "Dillon Francis",
    title: "When We Were Young",
    cover: "http://0.0.0.0:8000/es0r5Icy.jpg"
  },
  config: {
    duration: 20,
    source: {
      name: "test"
    }
  },
  player: {
    elapsed: 13.2,
    isPlaying: false
  }
};

function punchtop(state = initialState, action) {
  switch (action.type) {
    case SET_PLAYLIST: {
      const source = Object.assign({}, state.config.source, {
        name: action.name
      });
      const config = Object.assign({}, state.config, {
        source,
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
}

const rootReducer = history =>
  combineReducers({
    punchtop,
    router: connectRouter(history)
  });

export default rootReducer;
