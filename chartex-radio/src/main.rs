use std::time::{Duration, Instant};
use std::{env, fs};

use anyhow::anyhow;
use argh::FromArgs;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use songrec::fingerprinting::algorithm::SignatureGenerator;
use songrec::fingerprinting::communication::recognize_song_from_signature;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Recognize the currently playing track in a radio station.
#[derive(FromArgs)]
struct Args {
    /// enable debug messages
    #[argh(switch, short = 'd')]
    debug: bool,
    /// radio station URL
    #[argh(option, short = 's')]
    station: String,
    /// interval seconds between recognitions
    #[argh(option, short = 'i', default = "5")]
    interval: usize,
    /// endpoint to send the results
    #[argh(option, short = 'e')]
    endpoint: Option<String>,
    /// temporary file for the stream
    #[argh(option, short = 'o', default = "String::from(\"stream.out\")")]
    stream_file: String,
}

async fn start(args: &Args, client: &Client, stream_client: &Client) -> anyhow::Result<()> {
    let mut stream = stream_client
        .get(&args.station)
        .send()
        .await?
        .bytes_stream();
    let mut chunks = Vec::<u8>::new();
    let mut time = Instant::now();
    while let Some(chunk) = stream.next().await {
        log::info!("Reading bytes from the stream");
        let chunk = chunk?;
        chunks.extend(chunk.as_ref().to_vec());
        if time.elapsed().as_secs() < args.interval as u64 {
            continue;
        } else {
            time = Instant::now();
        }
        log::info!("Saving to a file");
        fs::write(&args.stream_file, &chunks)?;
        log::info!("Creating a signature");
        let signature = SignatureGenerator::make_signature_from_file(&args.stream_file)
            .map_err(|e| anyhow!("{}", e))?;
        log::info!("Attempting to recognize song");
        match recognize_song_from_signature(&signature) {
            Ok(mut song) => {
                log::info!("Song is recognized successfully");
                if let Some(song_object) = song.as_object_mut() {
                    song_object.insert(String::from("station"), Value::from(args.station.as_str()));
                    song_object.insert(String::from("time"), Value::from(Utc::now().to_rfc3339()));
                }
                log::debug!("{}", serde_json::to_string_pretty(&song)?);
                if let Some(endpoint) = args.endpoint.as_ref() {
                    log::info!("Sending a request to the endpoint");
                    match client.post(endpoint).json(&song).send().await {
                        Ok(response) => {
                            log::info!("Endpoint response: {}", response.status())
                        }
                        Err(e) => {
                            log::error!("Endpoint error: {e}");
                        }
                    }
                }
                chunks.clear();
            }
            Err(e) => {
                log::error!("Recognize error: {e}");
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();
    if args.debug {
        env::set_var("RUST_LOG", "debug");
    }
    log4rs::init_file("log4rs.yml", Default::default())?;
    log::info!("Starting...");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    loop {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .timeout(Duration::from_secs(120))
            .build()?;
        let stream_client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .http2_keep_alive_while_idle(true)
            .build()?;
        runtime.block_on(async {
            if let Err(e) = start(&args, &client, &stream_client).await {
                log::error!("Error occurred: {e}");
            }
        });
    }
}
