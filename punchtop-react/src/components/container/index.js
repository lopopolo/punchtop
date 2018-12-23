import React from "react";
import { connect } from "react-redux";

import style from "./style.css";
import DeviceDrawer from "../device-drawer";

const Container = ({ sourceName, cursor, children }) => (
  <div className={style.container}>
    <div className={style.header}>
      <div className={style.title}>{cursor}</div>
      <div className={style.sourceName}>{sourceName}</div>
    </div>
    {children}
    <DeviceDrawer />
  </div>
);

const mapStateToProps = state => ({
  sourceName: state.punchtop.config?.source?.name,
  cursor: state.punchtop.media?.current?.cursor
});

export default connect(mapStateToProps)(Container);
