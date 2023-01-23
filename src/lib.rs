pub mod fingerprinting {
    pub mod algorithm;
    pub mod communication;
    mod hanning;
    pub mod signature_format;
    mod user_agent;
}

pub mod core {
    pub mod http_thread;
    pub mod microphone_thread;
    pub mod processing_thread;
    pub mod thread_messages;
}

#[cfg(feature = "gui")]
pub mod gui {
    pub mod main_window;
    mod preferences;
    mod song_history_interface;
}

pub mod cli {
    pub mod cli_main;
}

pub mod utils {
    pub mod csv_song_history;
    pub mod ffmpeg_wrapper;
    #[cfg(feature = "gui")]
    pub mod filesystem_operations;
    pub mod internationalization;
    pub mod mpris_player;
    #[cfg(feature = "gui")]
    pub mod pulseaudio_loopback;
    pub mod thread;
}
