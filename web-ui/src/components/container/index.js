import dig from "object-dig";
import React from "react";
import { connect } from "react-redux";

import style from "./style.css";

const Container = ({ sourceName, cursor, children }) => (
  <div className={style.container}>
    <div className={style.header}>
      <div className={style.title}>{cursor}</div>
      <div className={style.sourceName}>{sourceName}</div>
    </div>
    {children}
  </div>
);

const mapStateToProps = state => ({
  sourceName: dig(state.punchtop.config, "source", "name"),
  cursor: dig(state.punchtop.media, "current", "cursor")
});

export default connect(mapStateToProps)(Container);
