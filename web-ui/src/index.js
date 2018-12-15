import React from "react";
import ReactDOM from "react-dom";

import {PlayerIcon} from "react-player-controls";

import './root.css';
import style from "./player.css";

const ElapsedBar = ({ elapsed, duration }) => (
  <div className={style.mediaScrubber}>
    <div className={style.mediaScrubberElapsed} style={{width: `${100* elapsed / duration}%`}} />
    <div className={style.mediaScrubberFill} />
  </div>
);

const Spacer = ({ height }) => <div style={{height}} />

const Index = () => <div>
    <div className={style.bg} />
    <div className={style.dim} />
    <div className={style.root}>
      <div className={style.container}>
        <div className={style.title}>Punchtop</div>
        <Spacer height="1.5em" />
        <img alt="Dillon Francis - When We Were Young album cover" className={style.cover} width="600" height="600" src="http://0.0.0.0:8000/es0r5Icy.jpg" />
        <Spacer height="1.5em" />
        <div className={style.metadata}>
          <div className={style.metadataTitle}>When We Were Young</div>
          <Spacer height="0.5em" />
          <div className={style.metadataArtist}>Dillon Francis</div>
        </div>
        <Spacer height="1.5em" />
        <div className={style.mediaPlayer}>
          <ElapsedBar elapsed={13.2} duration={20} />
          <Spacer height="1em" />
          <PlayerIcon.Play width={32} height={32} fill="lightgray" />
        </div>
      </div>
    </div>
  </div>;

ReactDOM.render(<Index />, document.getElementById("app"));
