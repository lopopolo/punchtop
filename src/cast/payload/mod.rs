pub mod connection {
    pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.connection";

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum Payload {
        Connect,
        Close,
    }
}

pub mod heartbeat {
    pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.heartbeat";

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum Payload {
        Ping,
        Pong,
    }
}

pub mod media {
    pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.media";

    const METADATA_TYPE_GENERIC: u32 = 0;
    const METADATA_TYPE_MOVIE: u32 = 1;
    const METADATA_TYPE_TV_SHOW: u32 = 2;
    const METADATA_TYPE_MUSIC_TRACK: u32 = 3;
    const METADATA_TYPE_PHOTO: u32 = 4;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum Payload {
        #[serde(rename_all = "camelCase")]
        GetStatus {
            request_id: i32,
            #[serde(skip_serializing_if = "Option::is_none")]
            media_session_id: Option<i32>,
        },
        #[serde(rename_all = "camelCase")]
        Load {
            request_id: i32,
            session_id: String,
            media: Media,
            current_time: f32,
            custom_data: CustomData,
            autoplay: bool,
        },
        #[serde(rename_all = "camelCase")]
        Play {
            request_id: i32,
            media_session_id: i32,
            custom_data: CustomData,
        },
        #[serde(rename_all = "camelCase")]
        Pause {
            request_id: i32,
            media_session_id: i32,
            custom_data: CustomData,
        },
        #[serde(rename_all = "camelCase")]
        Stop{
            request_id: i32,
            media_session_id: i32,
            custom_data: CustomData,
        },
        #[serde(rename_all = "camelCase")]
        Seek {
            request_id: i32,
            media_session_id: i32,
            resume_state: Option<String>,
            current_time: Option<f32>,
            custom_data: CustomData,
        },
        #[serde(rename_all = "camelCase")]
        MediaStatus {
            #[serde(default)]
            request_id: i32,
            status: Vec<Status>,
        },
        #[serde(rename_all = "camelCase")]
        LoadCancelled {
            request_id: i32,
        },
        #[serde(rename_all = "camelCase")]
        LoadFailed {
            request_id: i32,
        },
        #[serde(rename_all = "camelCase")]
        InvalidPlayerState {
            request_id: i32,
        },
        #[serde(rename_all = "camelCase")]
        InvalidRequest {
            request_id: i32,
            reason: Option<String>,
        },
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Media {
        pub content_id: String,
        #[serde(default)]
        pub stream_type: String,
        pub content_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub metadata: Option<Metadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub duration: Option<f32>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Metadata {
        pub metadata_type: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub series_title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub album_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub subtitle: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub album_artist: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub artist: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub composer: Option<String>,
        pub images: Vec<Image>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub release_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub original_air_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub creation_date_time: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub studio: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub location: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub latitude: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub longitude: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub season: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub episode: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub track_number: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub disc_number: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub height: Option<u32>,
    }

    impl Metadata {
        pub fn music_default() -> Metadata {
            Metadata {
                metadata_type: METADATA_TYPE_MUSIC_TRACK,
                title: None,
                series_title: None,
                album_name: None,
                subtitle: None,
                album_artist: None,
                artist: None,
                composer: None,
                images: Vec::new(),
                release_date: None,
                original_air_date: None,
                creation_date_time: None,
                studio: None,
                location: None,
                latitude: None,
                longitude: None,
                season: None,
                episode: None,
                track_number: None,
                disc_number: None,
                width: None,
                height: None,
            }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Image {
        pub url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub height: Option<u32>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CustomData {
        #[serde(skip_serializing)]
        private: (),
    }

    impl CustomData {
        pub fn new() -> CustomData {
            CustomData { private: () }
        }
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Status {
        pub media_session_id: i32,
        #[serde(default)]
        pub media: Option<Media>,
        pub playback_rate: f32,
        pub player_state: String,
        pub idle_reason: Option<String>,
        pub current_time: Option<f32>,
        pub supported_media_commands: u32,
    }
}

pub mod receiver {
    pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.receiver";

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum Payload {
        #[serde(rename_all = "camelCase")]
        Launch {
            request_id: i32,
            app_id: String,
        },
        #[serde(rename_all = "camelCase")]
        Stop {
            request_id: i32,
            session_id: String,
        },
        #[serde(rename_all = "camelCase")]
        GetStatus {
            request_id: i32,
        },
        #[serde(rename_all = "camelCase")]
        GetAppAvailability {
            request_id: i32,
            app_id: Vec<String>,
        },
        #[serde(rename_all = "camelCase")]
        ReceiverStatus {
            request_id: i32,
            status: Status,
        },
        SetVolume {
            volume: Volume,
        },
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Status {
        #[serde(default)]
        pub applications: Vec<Applications>,
        #[serde(default)]
        pub is_active_input: bool,
        pub volume: Volume,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct Applications {
        pub app_id: String,
        pub display_name: String,
        pub namespaces: Vec<Namespace>,
        pub session_id: String,
        pub status_text: String,
        pub transport_id: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Namespace {
        pub name: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Volume {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub level: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub muted: Option<bool>,
    }
}
