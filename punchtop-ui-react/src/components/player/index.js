import React from "react";
import { connect } from "react-redux";
import ReactCSSTransitionReplace from "react-css-transition-replace";
import format from "format-duration";

import style from "./style.css";
import Active from "./active";
import Idle from "./idle";
import { togglePlayback } from "../../actions";

export const Spacer = ({ height }) => <div style={{ height }} />;

export const FallbackCover = () => (
  <div className={style.fallbackCover}>
    <span role="img" aria-label="Album cover">
      🎵
    </span>
  </div>
);

export const ElapsedBar = ({ elapsed, duration }) => (
  <div className={style.time}>
    <div className={style.timestamps}>
      <div>{format(elapsed * 1000)}</div>
      <div>{format((elapsed - duration) * 1000)}</div>
    </div>
    <div className={style.scrubber}>
      <div
        className={style.elapsed}
        style={{ width: `${(100 * elapsed) / duration}%` }}
      />
      <div className={style.remaining} />
    </div>
  </div>
);

const Player = ({ media, ...props }) => (
  <div>
    <ReactCSSTransitionReplace
      transitionName="cross-fade"
      transitionEnterTimeout={300}
      transitionLeaveTimeout={300}
    >
      <div key={media ? "active" : "idle"}>
        {media ? <Active media={media} {...props} /> : <Idle {...props} />}
      </div>
    </ReactCSSTransitionReplace>
  </div>
);

const mapStateToProps = state => ({
  media: state.punchtop.media.current,
  isPlaying: state.punchtop.player.isPlaying,
  elapsed: state.punchtop.player.elapsed,
  duration: state.punchtop.config.duration
});

const mapDispatchToProps = dispatch => ({
  toggle: () => dispatch(togglePlayback())
});

export default connect(
  mapStateToProps,
  mapDispatchToProps
)(Player);
