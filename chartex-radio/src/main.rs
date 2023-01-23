use std::env;
use std::fs;
use std::time::Instant;

use futures_util::StreamExt;
use songrec::fingerprinting::algorithm::SignatureGenerator;
use songrec::fingerprinting::communication::recognize_song_from_signature;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let station = env::args().nth(1).expect("no radio station URL given");
    let interval = env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(5);
    let stream_file = env::args()
        .nth(3)
        .unwrap_or_else(|| String::from("stream.out"));
    let mut stream = reqwest::get(station).await?.bytes_stream();
    let mut chunks = Vec::<u8>::new();
    let mut time = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        chunks.extend(chunk.as_ref().to_vec());
        if time.elapsed().as_secs() < interval {
            continue;
        } else {
            time = Instant::now();
        }
        fs::write(&stream_file, &chunks).unwrap();
        match SignatureGenerator::make_signature_from_file(&stream_file) {
            Ok(signature) => match recognize_song_from_signature(&signature) {
                Ok(song) => {
                    println!("{}", serde_json::to_string_pretty(&song).unwrap());
                    chunks.clear();
                }
                Err(e) => {
                    eprintln!("{e}");
                }
            },
            Err(e) => {
                eprintln!("{e}");
            }
        }
    }
    Ok(())
}
