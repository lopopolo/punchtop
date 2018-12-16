import React from "react";
import ReactDOM from "react-dom";
import { createStore } from "redux";
import { Provider } from "react-redux";

import rootReducer from "./reducer";
import App from "./app";
import * as actions from "./actions";
import Root from "./components/root";

const store = createStore(rootReducer(), { punchtop: window.PUNCHTOP });

const render = () => {
  ReactDOM.render(
    <Root>
      <Provider store={store}>
        <App />
      </Provider>
    </Root>,
    document.getElementById("app")
  );
};

render();

global.actions = actions;
global.store = store;
