import React from "react";
import { PlayerIcon } from "react-player-controls";
import { connect } from "react-redux";

import style from "./style.css";
import { togglePlayback } from "../../actions";

const Spacer = ({ height }) => <div style={{height}} />

const ElapsedBar = ({ elapsed, duration }) => (
  <div className={style.scrubber}>
    <div className={style.elapsed} style={{width: `${100 * elapsed / duration}%`}} />
    <div className={style.remaining} />
  </div>
);

const Player = ({ artist, title, cover, isPlaying, elapsed, duration, toggle }) => <div>
    <img alt={`${artist} - ${title} album cover`} className={style.cover} width="600" height="600" src={cover} />
    <Spacer height="1.5em" />
    <div className={style.metadata}>
      <div className={style.title}>{title}</div>
      <Spacer height="0.5em" />
      <div className={style.artist}>{artist}</div>
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

const mapStateToProps = state => ({
  artist: state.punchtop.media.artist,
  title: state.punchtop.media.title,
  cover: state.punchtop.media.cover,
  isPlaying: state.punchtop.player.isPlaying,
  elapsed: state.punchtop.player.elapsed,
  duration: state.punchtop.config.duration,
});

const mapDispatchToProps = dispatch => ({
  toggle: () => dispatch(togglePlayback()),
})

export default connect(mapStateToProps, mapDispatchToProps)(Player);
