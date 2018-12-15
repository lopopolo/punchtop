import { combineReducers } from "redux";
import { connectRouter } from "connected-react-router";

import { TOGGLE_PLAYBACK } from "./actions";

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
    case TOGGLE_PLAYBACK: {
      const player = Object.assign({}, state.player, {
        isPlaying: !state.player.isPlaying
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
