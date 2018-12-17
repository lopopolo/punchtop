import React from "react";
import ReactDOM from "react-dom";
import { createStore } from "redux";
import { Provider } from "react-redux";

import rootReducer from "./reducer";
import App from "./app";
import Root from "./components/root";
import "./index.css";

const store = createStore(rootReducer());

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

global.store = store;
