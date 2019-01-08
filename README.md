# punchtop

punchtop is an audio game that runs a power hour using music on the local file
system and streams to a Chromecast device.

## Game

A [power hour](https://en.wikipedia.org/wiki/Power_hour) is a (drinking) game.
During each 60-second round, a song is played. A change in music marks each new
round.

## Usage

To run punchtop, you must build it from source. Punchtop depends on:

- nightly rust, which you can install using [rustup](https://rustup.rs/)
- node
- yarn

After you have installed the build dependencies, you can launch punchtop with
debug logging via cargo:

```sh
RUST_BACKTRACE=1 RUST_LOG=cast-client=debug,punchtop=debug,rocket=info caffeinate -s cargo run
```

## Limitations / Known Bugs

- Media directory may only be selected by modifying the
  [source](punchtop-webview/src/main.rs#L42-L48).
- Chromecast device may only be selected by modifying the
  [source](punchtop-webview/src/main.rs#L25).
- App does not prevent system sleep via idle timeout.
- [macOS] App does not shut down cleanly on quit.
- [macOS] App does not shut down cleanly on SIGINT.
- [macOS] App does not exit on game completion until webview has user activity.

## Screenshots

![Punchtop player](doc/player.png =250x)
