import React from "react";
import { connect } from "react-redux";
import ReactCSSTransitionReplace from "react-css-transition-replace";

import style from "./style.css";
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
    <ReactCSSTransitionReplace
      transitionName="cross-fade"
      transitionEnterTimeout={300}
      transitionLeaveTimeout={300}
    >
      <div key={active ? "active" : "idle"}>
        {active ? <Active /> : <Idle />}
      </div>
    </ReactCSSTransitionReplace>
  </div>;

const mapStateToProps = state => ({
    active: !!state.punchtop.media.current,
});

export default connect(mapStateToProps)(Player);
