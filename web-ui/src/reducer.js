import clamp from "clamp";
import { combineReducers } from "redux";
import { connectRouter } from "connected-react-router";

import {
  TOGGLE_PLAYBACK,
  SET_ACTIVE_TRACK,
  SET_ELAPSED,
  SET_PLAYLIST
} from "./actions";

const initialState = {
  media: {
    es0r5Icy: {
      artist: "Dillon Francis",
      title: "When We Were Young",
      cover: {
        height: 600,
        width: 600,
        url: "http://0.0.0.0:8000/es0r5Icy.jpg"
      }
    },
    "4FpLLFTy": {
      artist: "Tame Impala",
      cover: {
        height: 600,
        width: 600,
        url: "http://0.0.0.0:8000/4FpLLFTy.jpg"
      }
    },
    "": {
      cover: {
        height: 160,
        width: 160,
        url: "http://0.0.0.0:8000/musical-note_1f3b5.png"
      }
    }
  },
  config: {
    duration: 20,
    source: {
      name: "test"
    }
  },
  player: {
    current: "",
    elapsed: 13.2,
    isPlaying: false
  }
};

const reducer = (state = initialState, action) => {
  switch (action.type) {
    case SET_ACTIVE_TRACK: {
      const player = Object.assign({}, state.player, {
        current: action.id
      });
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
