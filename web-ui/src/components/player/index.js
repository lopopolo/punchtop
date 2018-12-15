import React from "react";
import { connect } from "react-redux";

import style from "./style.css";
import { togglePlayback } from "../../actions";
import Active from "./active";
import Idle from "./idle";

export const Spacer = ({ height }) => <div style={{height}} />

export const ElapsedBar = ({ elapsed, duration }) => (
  <div className={style.scrubber}>
    <div className={style.elapsed} style={{width: `${100 * elapsed / duration}%`}} />
    <div className={style.remaining} />
  </div>
);

const Player = ({ active }) => <div>
    {active ? <Active /> : <Idle />}
  </div>;

const mapStateToProps = state => ({
    active: !!state.punchtop.config.source,
});

export default connect(mapStateToProps)(Player);
