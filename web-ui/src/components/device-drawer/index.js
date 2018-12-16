import React from "react";
import { connect } from "react-redux";
import { withStyles } from "@material-ui/core/styles";
import Drawer from "@material-ui/core/Drawer";
import Button from "@material-ui/core/Button";
import List from "@material-ui/core/List";
import Divider from "@material-ui/core/Divider";
import ListItem from "@material-ui/core/ListItem";
import ListItemIcon from "@material-ui/core/ListItemIcon";
import ListItemText from "@material-ui/core/ListItemText";
import Cast from "@material-ui/icons/Cast";
import CastConnected from "@material-ui/icons/CastConnected";
import Computer from "@material-ui/icons/Computer";

import { setActiveDevice } from "../../actions";

const DEVICE_KIND_CAST = "cast";
const DEVICE_KIND_LOCAL = "local";

const styles = {
  root: {
    background: "transparent",
    "&:hover": {
      backgroundColor: "transparent"
    },
    borderRadius: 3,
    border: 0,
    color: "lightgray",
    height: 48,
    padding: 0
  },
  label: {
    textTransform: "capitalize"
  }
};

class DeviceDrawer extends React.Component {
  state = {
    open: false
  };

  toggleDrawer = open => () => {
    this.setState({
      open
    });
  };

  select = (kind, name) => () => {
    const { setActive } = this.props;
    this.toggleDrawer(false)();
    setActive(kind, name);
  };

  render() {
    const { active, classes, devices } = this.props;
    const { open } = this.state;

    const list = (
      <div>
        <List>
          {devices
            .filter(d => d.kind === DEVICE_KIND_CAST)
            .map(d => d.name)
            .sort()
            .map(name => (
              <ListItem
                button
                key={name}
                onClick={this.select(DEVICE_KIND_CAST, name)}
              >
                <ListItemIcon>
                  {active.type === DEVICE_KIND_CAST && active.name === name ? (
                    <CastConnected />
                  ) : (
                    <Cast />
                  )}
                </ListItemIcon>
                <ListItemText primary={name} />
              </ListItem>
            ))}
        </List>
        <Divider />
        <List>
          {devices
            .filter(d => d.kind === DEVICE_KIND_LOCAL)
            .map(d => d.name)
            .sort()
            .map(name => (
              <ListItem
                button
                key={name}
                onClick={this.select(DEVICE_KIND_LOCAL, name)}
              >
                <ListItemIcon>
                  <Computer />
                </ListItemIcon>
                <ListItemText primary={name} />
              </ListItem>
            ))}
        </List>
      </div>
    );

    return (
      <div>
        <Button
          classes={classes}
          fullWidth
          disableRipple
          disableFocusRipple
          onClick={this.toggleDrawer(true)}
        >
          Devices Available
        </Button>
        <Drawer anchor="bottom" open={open} onClose={this.toggleDrawer(false)}>
          <div tabIndex={0} role="button">
            {list}
          </div>
        </Drawer>
      </div>
    );
  }
}

const mapStateToProps = state => ({
  active: state.punchtop.device.active,
  devices: state.punchtop.device.all
});

const mapDispatchToProps = dispatch => ({
  setActive: (kind, name) => dispatch(setActiveDevice(kind, name))
});

export default withStyles(styles)(
  connect(
    mapStateToProps,
    mapDispatchToProps
  )(DeviceDrawer)
);
