import React from "react";
import lifecycle from "react-pure-lifecycle";

import Container from "./components/container";
import Player from "./components/player";

const methods = {
  componentDidMount() {
    global.external.invoke("init");
  }
};

const App = () => (
  <Container>
    <Player />
  </Container>
);

export default lifecycle(methods)(App);
