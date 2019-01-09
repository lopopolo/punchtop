# cast-client

Crate `cast-client` provides a client for controlling media playback on a
Chromecast device. It supports launching the _Default Media Receiver_ app,
loading media, controlling media playback, and modifying device settings.

## Device Discovery

Chromecast devices are discoverable on the local network using mDNS and the
`_googlecast._tcp.local` service name. The connection information for a device
is given by a DNS `A` or `AAAA` record (IP) and `SRV` record (port), The device
name can be extracted from the `fn` field of the DNS `TXT` record.

## Cast Protocol

### Transport

Communication with the device happens over a long-lived TLS socket. Host name
and cert verification on this connection is optional for clients.

The cast protocol is a framed protocol. Each frame consists of a `u32`
length-prefixed [`CastMessage` protobuf](proto/cast_channel.proto#L11-L52). The
maximum size of the encoded protobuf for requests and responses is 64KB.

### Channels

The `CastMessage` protobuf is used to multiplex messages over several
communication channels which are identified by the `namespace` field in the
protobuf. The available namespaces are:

- `urn:x-cast:com.google.cast.tp.connection`
- `urn:x-cast:com.google.cast.tp.heartbeat`
- `urn:x-cast:com.google.cast.media`
- `urn:x-cast:com.google.cast.receiver`

JSON-encoded messages are passed to each channel via the `payload_utf8` field in
the protobuf.

JSON payloads for the media and receiver channels are identified by a unique
request ID. The `0` request ID is reserved for "spontaneous" messages from the
device. When a payload generates a response from the receiver, the same request
ID will be echoed back in the response.

#### connection

The connection channel is a transport channel used to manage connections to
receiver apps.

##### Messages

###### Connect

**Purpose**: Establish a connection to the device or a transport channel for a
launched app.

```json
{
  "type": "CONNECT",
  "userAgent": "cast-client v0.1.0"
}
```

##### Responses

###### Close

**Purpose**: Device has closed the connection.

The connection may be closed due to a protocol error or liveness timeout. It may
be necessary to send a `CONNECT` message to reestablish the connection.

```json
{
  "type": "CLOSE"
}
```

#### heartbeat

The heartbeat channel is a transport channel used by the device and the client
for keepalive messages.

##### Messages

###### Ping

**Purpose**: Send a liveness challenge to the device.

A `PING` should be sent to the device on a regular interval. `cast-client` sends
a `PING` every 5 seconds.

```json
{
  "type": "PING"
}
```

###### Pong

**Purpose**: Acknowledge a liveness challenge from the device.

```json
{
  "type": "PONG"
}
```

##### Responses

###### Ping

**Purpose**: A liveness challenge from the device.

Should respond to the device with a `PONG` message.

```json
{
  "type": "PING"
}
```

###### Pong

**Purpose**: A liveness acknowledgement from the device.

No special handling is required.

```json
{
  "type": "PONG"
}
```

#### media

The media channel is used to control media playback.

The `customData` field on some messages is an optional JSON object that may be
interpreted by a custom
[receiver application](https://developers.google.com/cast/v2/receiver_apps).

`requestId` and `mediaSessionId` are `u64` fields.

##### Messages

###### Get Status

**Purpose**: Get playback status.

The device responds to this message with a `MEDIA_STATUS` object.

```json
{
  "type": "GET_STATUS",
  "requestId": 128403142794100773,
  "mediaSessionId": 8373237555663464450
}
```

The `mediaSessionId` field is optional; the field should be omitted from the
encoded JSON if there is no media session.

_Google Cast developer docs_:

`GET_STATUS`:
<https://developers.google.com/cast/docs/reference/messages#GetStatus>

###### Load

**Purpose**: Enqueue media for playback.

```json
{
  "type": "LOAD",
  "requestId": 2199981871899796657,
  "sessionId": "505EE05E-EB09-4030-A1CD-462CE256E7CB",
  "media": {
    "contentId": "http://www.example.com/song.mp3",
    "streamType": "NONE",
    "contentType": "audio/mp3",
    "metadata": {
      "metadataType": 3,
      "title": "Example Song"
    },
    "duration": 60.0,
    "customData": {}
  },
  "currentTime": 5.64,
  "customData": {},
  "autoplay": false
}
```

Valid values for `streamType` are: `NONE`, `BUFFERED`, `LIVE`.

_Google Cast developer docs_:

`LOAD`: <https://developers.google.com/cast/docs/reference/messages#Load>
`MediaInformation`:
<https://developers.google.com/cast/docs/reference/messages#MediaInformation>
Metadata:
[Generic](https://developers.google.com/cast/docs/reference/messages#GenericMediaMetadata),
[Movie](https://developers.google.com/cast/docs/reference/messages#MovieMediaMetadata),
[TV Show](https://developers.google.com/cast/docs/reference/messages#TvShowMediaMetadata),
[Music Track](https://developers.google.com/cast/docs/reference/messages#MusicTrackMediaMetadata),
[Photo](https://developers.google.com/cast/docs/reference/messages#PhotoMediaMetadata).

###### Play

**Purpose**: Set media playback state to playing.

After sending a `PLAY` message to the receiver, issue a `GET_STATUS` command to
verify playback state has been changed.

```json
{
  "type": "PLAY",
  "requestId": 8069653855621172357,
  "mediaSessionId": 16690720058263264245,
  "customData": {}
}
```

_Google Cast developer docs_:

`PLAY`: <https://developers.google.com/cast/docs/reference/messages#Play>

###### Pause

**Purpose**: Set media playback state to paused.

After sending a `PAUSE` message to the receiver, issue a `GET_STATUS` command to
verify playback state has been changed.

```json
{
  "type": "PAUSE",
  "requestId": 8069653855621172357,
  "mediaSessionId": 16690720058263264245,
  "customData": {}
}
```

_Google Cast developer docs_:

`PAUSE`: <https://developers.google.com/cast/docs/reference/messages#Pause>

###### Stop

**Purpose**: Set media playback state to stopped.

After sending a `STOP` message to the receiver, issue a `GET_STATUS` command to
verify playback state has been changed.

```json
{
  "type": "STOP",
  "requestId": 6856272176370532247,
  "mediaSessionId": 1730523897409722602,
  "customData": {}
}
```

_Google Cast developer docs_:

`STOP`: <https://developers.google.com/cast/docs/reference/messages#Stop>

###### Seek

**Purpose**: Set playback position.

After sending a `SEEK` message to the receiver, issue a `GET_STATUS` command to
verify playback position has been changed.

```json
{
  "type": "SEEK",
  "requestId": 17130378735599745281,
  "mediaSessionId": 4781177872522835899,
  "resumeState": "PLAYBACK_START",
  "currentTime": 42.42,
  "customData": {}
}
```

Valid values for `resumeState` are: `PLAYBACK_START`, `PLAYBACK_PAUSE`.

_Google Cast developer docs_:

`SEEK`: <https://developers.google.com/cast/docs/reference/messages#Seek>

##### Responses

###### Media Status

**Purpose**: Describe media playback state.

```json
{
  "requestId": 17130378735599745281,
  "status: [
    {
      "mediaSessionId": 4781177872522835899,
      "media": {
        "contentId": "http://www.example.com/song.mp3",
        "streamType": "NONE",
        "contentType": "audio/mp3",
        "metadata": {
          "metadataType": 3,
          "title": "Example Song"
        },
        "duration": 60.0,
        "customData": {}
      },
      "playbackRate": 1.0,
      "playerState": "PLAYING",
      "currentTime": 42.42,
      "supportedMediaCommands": 63
    }
  ]
}
```

Valid values for `playerState` are: `IDLE`, `PLAYING`, `BUFFERING`, `PAUSED`.
`supportedMediaCommands` is a bitmask with the following flags:

```
1   Pause
2   Seek
4   Stream volume
8   Stream mute
16  Skip forward
32  Skip backward
```

_Google Cast developer docs_:

Media Status:
<https://developers.google.com/cast/docs/reference/messages#MediaStatus>

###### Load Cancelled

**Purpose**: Error message indicating a load was cancelled because a second
request was received.

```json
{
  "type": "LOAD_CANCELLED",
  "requestId": 9423939210460905955,
  "customData": {}
}
```

_Google Cast developer docs_:

`LOAD_CANCELLED`:
<https://developers.google.com/cast/docs/reference/messages#LoadCancelled>

###### Load Failed

**Purpose**: Error message indicating a load failed. The `playerState` will be
`IDLE`.

```json
{
  "type": "LOAD_FAILED",
  "requestId": 10576902510017753157,
  "customData": {}
}
```

_Google Cast developer docs_:

`LOAD_FAILED`:
<https://developers.google.com/cast/docs/reference/messages#LoadFailed>

###### Invalid Player State

**Purpose**: Error message indicating an action cannot be performed because the
player is in an invalid state (e.g. attempting to perform a `SEEK` when no media
is loaded).

```json
{
  "type": "INVALID_PLAYER_STATE",
  "requestId": 10364330086991706802,
  "customData": {}
}
```

_Google Cast developer docs_:

`INVALID_PLAYER_STATE`:
<https://developers.google.com/cast/docs/reference/messages#InvalidPlayerState>

###### Invalid Request

**Purpose**: Error message indicating an the request is invalid or cannot be
completed.

```json
{
  "type": "INVALID_REQUEST",
  "requestId": 8000646305415193525,
  "reason": "INVALID_COMMAND",
  "customData": {}
}
```

Valid values for `reason` are: `INVALID_COMMAND`, `DUPLICATE_REQUEST_ID`.

_Google Cast developer docs_:

`INVALID_REQUEST`:
<https://developers.google.com/cast/docs/reference/messages#InvalidRequest>

#### receiver

The receiver channel is used to control device state.

##### Messages

###### Launch

**Purpose**: Launch an app on the device.

Media is played by an app; you must launch an app before issuing a `LOAD`.

```json
{
  "type": "LAUNCH",
  "requestId": 10181705186791964602,
  "appId": "CC1AD845"
}
```

`CC1AD845` is the `appId` for the _default media receiver_ app.

###### Get Status

**Purpose**: Get device status.

The device responds to this message with a `RECEIVER_STATUS` object.

```json
{
  "type": "GET_STATUS",
  "requestId": 18205553929436936635
}
```

###### Get App Availability

**Purpose**: Query if the device can launch the provided `appId`s.

The device responds to this message with a `RECEIVER_STATUS` object.

```json
{
  "type": "GET_APP_AVAILABILITY",
  "requestId": 16619677927068003483,
  "appId": ["CC1AD845"]
}
```

###### Set Volume

**Purpose**: Set the volume on the device.

```json
{
  "type": "SET_VOLUME",
  "volume": {
    "level": 0.75,
    "muted": false
  }
}
```

Both fields in the `volume` object are optional. When not provided, the property
is left unmodified.

##### Responses

###### Receiver Status

**Purpose**: Describe receiver state.

```json
{
  "type": "RECEIVER_STATUS",
  "requestId": 18205553929436936635,
  "status": {
    "applications" [
      {
        "appId": "CC1AD845",
        "displayName": "Default Media Receiver",
        "namespaces" [
          "urn:x-cast:com.google.cast.tp.connection",
          "urn:x-cast:com.google.cast.tp.heartbeat",
          "urn:x-cast:com.google.cast.media",
          "urn:x-cast:com.google.cast.receiver"
        ],
        "sessionId": "3E8F3FEF-C420-42E3-A3AC-1FB4EFC2E0CD",
        "statusText": "Playing",
        "transportId": "505EE05E-EB09-4030-A1CD-462CE256E7CB"
      }
    ],
  }
}
```

### Session and Transport

Upon connecting to the device, senders must initiate connections with the apps
they launch. For messages targeted at a launched app, the `transportId` of the
app is the `destination`.

### Sender and Receiver Multiplexing

The cast protocol includes multiple concepts for multiplexing over the shared
TLS socket.

#### Source

Every `CastMessage` must send a `source` which specifies the sender (client) ID.
There is a special source for default senders, `sender-0`.

For all messages on all channels, it is sufficient to send the
`DEFAULT_SENDER_ID`.

#### Destination

Every `CastMessage` must set a `destination` which specifies the _target app_.
There is a special destination for messages directed at the receiver,
`receiver-0`.

- For messages on the connection channel, the `destination` is either the
  `DEFAULT_DESTINATION_ID` for initial connection or the `transportId` for a
  launched app.
- For messages on the heartbeat channel, the `destination` is the
  `DEFUALT_DESTINATION_ID`.
- For messages on the media channel, the `destination` is the `transportId` of
  the target launched app.
- For messages on the receiver channel, the `destination` is the
  `DEFUALT_DESTINATION_ID`.

## Streaming Media to a Device

The `LOAD` command on the media channel loads a media by URL. In order to serve
media to a Chromecast device, the device must be able to stream it. When
attempting to play local media (i.e. a song or video on disk), the sender must
make it accessible to the device by running an embedded media server.

Make each item in the playlist accessible via a unique URL. This URL only needs
to be stable during playback of the item.

### Supported Media Formats

<https://developers.google.com/cast/docs/media>

Chromecasts support a variety of media formats. If attempting to play media in a
format the device does not support, the sender can transcode on-the-fly to a
format supported by the device and make the transcode available via the embedded
media server.
