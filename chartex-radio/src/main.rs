use std::fs;
use std::time::{Duration, Instant};

use argh::FromArgs;
use chrono::Utc;
use futures_util::StreamExt;
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();
    let client = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(0)
        .build()?;
    let mut stream = reqwest::Client::builder()
        .http2_keep_alive_while_idle(true)
        .pool_idle_timeout(None)
        .pool_max_idle_per_host(usize::MAX)
        .build()?
        .get(&args.station)
        .send()
        .await?
        .bytes_stream();
    let mut chunks = Vec::<u8>::new();
    let mut time = Instant::now();
    println!("Starting...");
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        chunks.extend(chunk.as_ref().to_vec());
        if time.elapsed().as_secs() < args.interval as u64 {
            continue;
        } else {
            time = Instant::now();
        }
        fs::write(&args.stream_file, &chunks)?;
        match SignatureGenerator::make_signature_from_file(&args.stream_file) {
            Ok(signature) => match recognize_song_from_signature(&signature) {
                Ok(mut song) => {
                    if let Some(song_object) = song.as_object_mut() {
                        song_object
                            .insert(String::from("station"), Value::from(args.station.as_str()));
                        song_object
                            .insert(String::from("time"), Value::from(Utc::now().to_rfc3339()));
                    }
                    if args.debug {
                        println!("{}", serde_json::to_string_pretty(&song)?);
                    }
                    if let Some(endpoint) = args.endpoint.as_ref() {
                        match client.post(endpoint).json(&song).send().await {
                            Ok(response) => {
                                if args.debug {
                                    println!("Endpoint response: {}", response.status())
                                }
                            }
                            Err(e) => {
                                eprintln!("Endpoint error: {e}");
                            }
                        }
                    }
                    chunks.clear();
                }
                Err(e) => {
                    eprintln!("Recognize error: {e}");
                }
            },
            Err(e) => {
                eprintln!("Signature error: {e}");
            }
        }
    }
    Ok(())
}
