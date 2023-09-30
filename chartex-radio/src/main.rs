use anyhow::anyhow;
use argh::FromArgs;
use chrono::Utc;
use futures_util::StreamExt;
use m3u8_rs::Playlist;
use reqwest::Client;
use serde_json::Value;
use songrec::fingerprinting::algorithm::SignatureGenerator;
use songrec::fingerprinting::communication::recognize_song_from_signature;
use std::time::{Duration, Instant};
use std::{env, fs};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Recognize the currently playing track in a radio station.
#[derive(Clone, FromArgs)]
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

async fn recognize(args: &Args, station: &str, client: &Client) -> Result<(), anyhow::Error> {
    log::info!("Creating a signature");
    let signature = SignatureGenerator::make_signature_from_file(&args.stream_file)
        .map_err(|e| anyhow!("{}", e))?;
    log::info!("Attempting to recognize song");
    let mut song = recognize_song_from_signature(&signature).map_err(|e| anyhow!("{}", e))?;
    log::info!("Song is recognized successfully");
    if let Some(song_object) = song.as_object_mut() {
        song_object.insert(String::from("station"), Value::from(station.trim()));
        song_object.insert(String::from("time"), Value::from(Utc::now().to_rfc3339()));
    }
    log::info!("{}", serde_json::to_string(&song)?);
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
    Ok(())
}

async fn start(
    args: &Args,
    station: &str,
    client: &Client,
    stream_client: &Client,
    tx: &UnboundedSender<()>,
    is_file: bool,
) -> anyhow::Result<()> {
    if is_file {
        log::info!("Reading bytes from the file");
        let bytes = stream_client
            .get(&args.station)
            .send()
            .await?
            .bytes()
            .await?;
        log::info!("Saving to a file");
        fs::write(&args.stream_file, &bytes)?;
        recognize(args, station, client).await?;
        tokio::time::sleep(Duration::from_secs(args.interval as u64)).await;
        tx.send(())?;
    } else {
        let mut stream = stream_client
            .get(&args.station)
            .send()
            .await?
            .bytes_stream();
        let mut chunks = Vec::<u8>::new();
        let time = Instant::now();
        log::info!("Reading bytes from stream");
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            chunks.extend(chunk.as_ref().to_vec());
            if time.elapsed().as_secs() < args.interval as u64 {
                continue;
            }
            log::info!("Saving to a file");
            fs::write(&args.stream_file, &chunks)?;
            recognize(args, station, client).await?;
            tx.send(())?;
            break;
        }
    }
    Ok(())
}

fn parse_station(uri: &str, station: &str) -> anyhow::Result<String> {
    let station = if uri.starts_with(":///") {
        let url = Url::parse(station)?;
        url.host_str()
            .and_then(|v| Url::parse(&format!("{}://{v}", url.scheme())).ok())
            .unwrap()
            .join(uri.trim_start_matches(":///"))
    } else if uri.starts_with("https://") {
        Url::parse(uri)
    } else {
        Url::parse(station)?.join(uri)
    }?
    .to_string();
    log::info!("Parsed station: {station}");
    Ok(station)
}

fn main() -> anyhow::Result<()> {
    let mut args: Args = argh::from_env();
    if args.debug {
        env::set_var("RUST_LOG", "debug");
    }
    log4rs::init_file("log4rs.yml", Default::default())?;
    log::info!("Starting...");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(20 * 1024 * 1024)
        .build()?;
    let task_timeout = (args.interval as u64 * 2 * 1000) + (60 * 1000);
    runtime.block_on(async {
        let mut is_file = false;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let stream_client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .http2_keep_alive_while_idle(true)
            .build()?;
        let original_station = args.station.clone();
        loop {
            if original_station.ends_with(".m3u8") {
                let mut station_url = original_station.clone();
                loop {
                    let bytes = stream_client
                        .get(&station_url)
                        .send()
                        .await?
                        .bytes()
                        .await?;
                    match m3u8_rs::parse_playlist_res(&bytes) {
                        Ok(Playlist::MediaPlaylist(pl)) => {
                            log::info!("{:#?}", pl);
                            let playlist = pl.segments.first().unwrap();
                            args.station = parse_station(&playlist.uri, &station_url)?;
                            args.interval = playlist.duration as usize;
                            log::info!("Station URL: {:#?}", args.station);
                            is_file = true;
                            break;
                        }
                        Ok(Playlist::MasterPlaylist(pl)) => {
                            log::info!("{:#?}", pl);
                            let uri = pl.variants.first().unwrap().uri.to_string();
                            station_url = parse_station(&uri, &args.station)?;
                        }
                        Err(e) => {
                            log::error!("{e}");
                            break;
                        }
                    }
                }
            } else if original_station.ends_with(".m3u") {
                let text = stream_client
                    .get(&original_station)
                    .send()
                    .await?
                    .text()
                    .await?;
                args.station = text.lines().next().unwrap().to_string();
                log::info!("Station URL: {:#?}", args.station);
            }
            let client = Client::builder()
                .user_agent(APP_USER_AGENT)
                .timeout(Duration::from_secs(120))
                .build()?;
            let stream_client = Client::builder()
                .user_agent(APP_USER_AGENT)
                .http2_keep_alive_while_idle(true)
                .build()?;
            let args_cloned = args.clone();
            let tx_cloned = tx.clone();
            let original_station = original_station.clone();
            let main_task = tokio::spawn(async move {
                if let Err(e) = start(
                    &args_cloned,
                    &original_station,
                    &client,
                    &stream_client,
                    &tx_cloned,
                    is_file,
                )
                .await
                {
                    log::error!("Error occurred: {e}");
                    if let Err(e) = tx_cloned.send(()) {
                        log::error!("Send error: {e}");
                    }
                }
            });
            log::info!("New task spawned");
            let tx_cloned = tx.clone();
            let timeout_task = tokio::spawn(async move {
                log::info!("Waiting for {}ms", task_timeout);
                tokio::time::sleep(Duration::from_millis(task_timeout)).await;
                log::warn!("Task timed out!");
                main_task.abort();
                tx_cloned.send(()).unwrap();
            });
            rx.recv().await;
            timeout_task.abort();
        }
    })
}
