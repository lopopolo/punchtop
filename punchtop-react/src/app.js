import React from "react";

import Container from "./components/container";
import Player from "./components/player";

class App extends React.Component {
  componentDidMount() {
    global.external.invoke("init");
  }

  render() {
    return (
      <Container>
        <Player />
      </Container>
    );
  }
}

export default App;
