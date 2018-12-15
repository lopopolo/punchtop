import React from "react";
import { Route, Switch } from "react-router";
import { ConnectedRouter } from "connected-react-router";

import Container from "./components/container";
import Player from "./components/player";

const App = ({ history }) => (
  <ConnectedRouter history={history}>
    <Container>
      <Switch>
        <Route path="/" component={Player} />
      </Switch>
    </Container>
  </ConnectedRouter>
);

export default App;
