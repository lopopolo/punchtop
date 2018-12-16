import React from "react";
import { connect } from "react-redux";
import ReactCSSTransitionReplace from "react-css-transition-replace";

import style from "./style.css";
import Active from "./active";
import Idle from "./idle";

export const Spacer = ({ height }) => <div style={{ height }} />;

export const ElapsedBar = ({ elapsed, duration }) => (
  <div className={style.scrubber}>
    <div
      className={style.elapsed}
      style={{ width: `${(100 * elapsed) / duration}%` }}
    />
    <div className={style.remaining} />
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

export default connect(mapStateToProps)(Player);
