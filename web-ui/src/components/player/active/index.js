import React from "react";
import { PlayerIcon } from "react-player-controls";
import { connect } from "react-redux";
import ReactCSSTransitionReplace from "react-css-transition-replace";

import style from "./style.css";
import { togglePlayback } from "../../../actions";
import { ElapsedBar, Spacer } from "..";

const Player = ({ id, media, isPlaying, elapsed, duration, toggle }) => <div>
    <ReactCSSTransitionReplace
      transitionName="cross-fade"
      transitionEnterTimeout={300}
      transitionLeaveTimeout={300}
    >
      <div key={id} className={style.coverContainer}>
        <img alt={[media.artist, media.title].filter(item => item).join(" - ")} className={style.cover} height={media.cover.height} width={media.cover.width} src={media.cover.url} />
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
      <ElapsedBar elapsed={elapsed} duration={duration} />
      <Spacer height="1em" />
      <button className={style.toggle} type="button" onClick={toggle}>
        {isPlaying ? <PlayerIcon.Pause width={32} height={32} fill="lightgray" /> : <PlayerIcon.Play width={32} height={32} fill="lightgray" />}
      </button>
    </div>
  </div>;

const mapStateToProps = state => {
  const id = state.punchtop.player.current;
  return {
    id,
    media: state.punchtop.media[id],
    isPlaying: state.punchtop.player.isPlaying,
    elapsed: state.punchtop.player.elapsed,
    duration: state.punchtop.config.duration,
  }
};

const mapDispatchToProps = dispatch => ({
  toggle: () => dispatch(togglePlayback()),
})

export default connect(mapStateToProps, mapDispatchToProps)(Player);
