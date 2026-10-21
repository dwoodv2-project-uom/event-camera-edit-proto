use clap::Parser;
use io::{
    aedat4_decoder::Aedat4,
    codec::{Decoder as _, DecoderFactory as _, Packet},
};
use std::io::Error;
use tokio::fs::File;

#[derive(Debug, Parser)]
struct Cli {
    /// Input file to read
    file: String,
    ts1: i64,
    ts2: i64,
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let cli = Cli::parse();
    let file = File::open(&cli.file).await?;

    let mut decoder = Aedat4::open(&Aedat4, file)
        .await
        .map_err(|err| Error::other(format!("error opening file: {err:?}")))?;

    println!("file info: {:?}", decoder.metadata);
    println!(
        "ts range: {}-{}",
        decoder.file_data_table.ts_start, decoder.file_data_table.ts_end
    );
    println!("total events: {}", decoder.file_data_table.total_events);
    println!("total frames: {}", decoder.file_data_table.total_frames);
    println!("total imu: {}", decoder.file_data_table.total_imu);
    println!("total trig: {}", decoder.file_data_table.total_trig);
    println!(
        "Events/s: {}",
        decoder.file_data_table.total_events as f64
            / (decoder.file_data_table.ts_end - decoder.file_data_table.ts_start) as f64
            * 1e9
    );

    let packets = decoder.packets_by_ts_range(cli.ts1, cli.ts2).await.unwrap();
    if let Some(vec) = packets {
        for packet in vec {
            match packet {
                Packet::EventPacket(event) => {
                    println!("{:?}", event.as_events());
                }
                Packet::FramePacket(_frame) => {
                    //println!("{:?}", frame.as_frame());
                }
                Packet::ImuPacket(_imu) => {
                    //println!("{:?}", imu.as_imu())
                }
                Packet::TriggerPacket(_trigger) => {
                    //println!("{:?}", trigger.as_trigger());
                }
                _ => {}
            }
        }
    }

    Ok(())
}
