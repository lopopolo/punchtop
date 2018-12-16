import React from "react";
import { PlayerIcon } from "react-player-controls";
import { connect } from "react-redux";
import ReactCSSTransitionReplace from "react-css-transition-replace";
import Img from "react-image";

import style from "./style.css";
import { togglePlayback } from "../../../actions";
import { ElapsedBar, Spacer } from "..";
import cover from "../../../assets/idle-cover.png";

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
          src={[media.cover && media.cover.url, cover]}
        />
      </div>
    </ReactCSSTransitionReplace>
    <Spacer height="1.5em" />
    <div className={style.metadata}>
      <div className={style.title}>{media.title}</div>
      <Spacer height="0.5em" />
      <div className={style.artist}>{media.artist}</div>
    </div>
    <Spacer height="1.5em" />
    <div className={style.player}>
      <ElapsedBar key={media.id} elapsed={elapsed} duration={duration} />
      <Spacer height="1em" />
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

const mapDispatchToProps = dispatch => ({
  toggle: () => dispatch(togglePlayback())
});

export default connect(mapDispatchToProps)(Player);
