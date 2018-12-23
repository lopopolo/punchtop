import React from "react";
import ReactCSSTransitionReplace from "react-css-transition-replace";
import Img from "react-image";
import { PlayerIcon } from "react-player-controls";
import { connect } from "react-redux";
import format from "format-duration";

import style from "./style.css";
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

const Player = ({ media, isPlaying, elapsed, duration, toggle }) => (
  <div>
    <ReactCSSTransitionReplace
      transitionName="cross-fade"
      transitionEnterTimeout={300}
      transitionLeaveTimeout={300}
    >
      <div key={media ? "active" : "idle"}>
        <div>
          <ReactCSSTransitionReplace
            transitionName="cross-fade"
            transitionEnterTimeout={300}
            transitionLeaveTimeout={300}
          >
            <div key={media?.id || "fallback"} className={style.coverContainer}>
              <Img
                alt={
                  [media?.artist, media?.title]
                    .filter(item => item)
                    .join(" - ") || "Punchtop"
                }
                className={style.cover}
                src={[media?.cover?.url]}
                unloader={<FallbackCover />}
              />
            </div>
          </ReactCSSTransitionReplace>
          <Spacer height="0.75em" />
          <div className={style.metadata}>
            <div className={style.title}>{media?.title}</div>
            <Spacer height="0.4em" />
            <div className={style.artist}>{media?.artist}</div>
          </div>
          <Spacer height="0.75em" />
          <div className={style.player}>
            <ElapsedBar
              key={media?.id || "fallback"}
              elapsed={elapsed}
              duration={duration}
            />
            <Spacer height="0.75em" />
            <button
              className={style.toggle}
              type="button"
              onClick={toggle}
              disabled={!media}
            >
              {isPlaying ? (
                <PlayerIcon.Pause width={32} height={32} fill="lightgray" />
              ) : (
                <PlayerIcon.Play width={32} height={32} fill="lightgray" />
              )}
            </button>
          </div>
        </div>
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
