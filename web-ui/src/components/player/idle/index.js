import React from "react";
import { PlayerIcon } from "react-player-controls";
import { connect } from "react-redux";

import style from "./style.css";
import { ElapsedBar, Spacer } from "..";
import cover from "../../../assets/idle-cover.png";

const Player = ({ duration }) => (
  <div>
    <div className={style.coverContainer}>
      <img
        alt="Punchtop"
        className={style.cover}
        height={160}
        width={160}
        src={cover}
      />
    </div>
    <Spacer height="1.5em" />
    <div className={style.metadata}>
      <div className={style.title} />
      <Spacer height="0.5em" />
      <div className={style.artist} />
    </div>
    <Spacer height="1.5em" />
    <div className={style.player}>
      <ElapsedBar elapsed={0} duration={duration} />
      <Spacer height="1em" />
      <button className={style.toggle} type="button" disabled>
        <PlayerIcon.Play width={32} height={32} fill="gray" />
      </button>
    </div>
  </div>
);

const mapStateToProps = state => ({
  duration: state.punchtop.config.duration
});

export default connect(mapStateToProps)(Player);
