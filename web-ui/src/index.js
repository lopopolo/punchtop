import React from "react";
import ReactDOM from "react-dom";
import { applyMiddleware, compose, createStore } from "redux";
import { createBrowserHistory } from "history";
import { routerMiddleware } from "connected-react-router";
import { Provider } from "react-redux";

import rootReducer from "./reducer";
import App from "./app";
import * as actions from "./actions";
import Root from "./components/root";

const history = createBrowserHistory();

const composeEnhancer = window.__REDUX_DEVTOOLS_EXTENSION_COMPOSE__ || compose;
const store = createStore(
  rootReducer(history),
  { punchtop: window.PUNCHTOP },
  composeEnhancer(applyMiddleware(routerMiddleware(history)))
);

const render = () => {
  ReactDOM.render(
    <Root>
      <Provider store={store}>
        <App history={history} />
      </Provider>
    </Root>,
    document.getElementById("app")
  );
};

render();

global.actions = actions;
global.store = store;
