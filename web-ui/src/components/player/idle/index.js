import React from "react";
import { PlayerIcon } from "react-player-controls";
import Img from "react-image";

import style from "./style.css";
import { ElapsedBar, Spacer } from "..";
import fallbackCover from "../../../assets/fallback-cover.png";

const Player = ({ duration }) => (
  <div>
    <div className={style.coverContainer}>
      <Img alt="Punchtop" className={style.cover} src={[fallbackCover]} />
    </div>
    <Spacer height="0.75em" />
    <div className={style.metadata}>
      <div className={style.title} />
      <Spacer height="0.4em" />
      <div className={style.artist} />
    </div>
    <Spacer height="0.75em" />
    <div className={style.player}>
      <ElapsedBar elapsed={0} duration={duration} />
      <Spacer height="0.75em" />
      <button className={style.toggle} type="button" disabled>
        <PlayerIcon.Play width={32} height={32} fill="gray" />
      </button>
    </div>
  </div>
);

export default Player;
