import React from "react";
import { PlayerIcon } from "react-player-controls";
import ReactCSSTransitionReplace from "react-css-transition-replace";
import Img from "react-image";

import style from "./style.css";
import { ElapsedBar, FallbackCover, Spacer } from "..";

const Player = ({ media, isPlaying, elapsed, duration, toggle }) => (
  <div>
    <ReactCSSTransitionReplace
      transitionName="cross-fade"
      transitionEnterTimeout={300}
      transitionLeaveTimeout={300}
    >
      <div key={media.id} className={style.coverContainer}>
        <Img
          alt={[media.artist, media.title].filter(item => item).join(" - ")}
          className={style.cover}
          src={[media.cover && media.cover.url]}
          unloader={<FallbackCover />}
        />
      </div>
    </ReactCSSTransitionReplace>
    <Spacer height="0.75em" />
    <div className={style.metadata}>
      <div className={style.title}>{media.title}</div>
      <Spacer height="0.4em" />
      <div className={style.artist}>{media.artist}</div>
    </div>
    <Spacer height="0.75em" />
    <div className={style.player}>
      <ElapsedBar key={media.id} elapsed={elapsed} duration={duration} />
      <Spacer height="0.75em" />
      <button className={style.toggle} type="button" onClick={toggle}>
        {isPlaying ? (
          <PlayerIcon.Pause width={32} height={32} fill="lightgray" />
        ) : (
          <PlayerIcon.Play width={32} height={32} fill="lightgray" />
        )}
      </button>
    </div>
  </div>
);

export default Player;
