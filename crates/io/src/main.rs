use io::{aedat4_decoder::Aedat4, codec::DecoderFactory};
use std::io::Error;
use tokio::fs::File;
use io::codec::Packet;
use io::event::size_prefixed_root_as_event_packet;

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    // TODO: REMOVE
    let file =
        File::open("/home/david/Github/event-camera-edit-proto/crates/io/src/davis346.aedat4")
            .await?;

    let mut decoder = Aedat4::open(&Aedat4, Box::pin(file)).await.map_err(|err| {
        Error::new(
            std::io::ErrorKind::Other,
            format!("error opening file: {:?}", err),
        )
    })?;

    loop {
        let packet = decoder.next_packet().await.expect("error decoding packet").unwrap();

        match packet {
            Packet::EventPacket(content) => {
                let event = size_prefixed_root_as_event_packet(&*content.buffer).unwrap();
                println!("event: {:?}", event);
            }
            Packet::FramePacket(_) => {}
            Packet::ImuPacket(_) => {}
            Packet::TriggerPacket(_) => {}
        }
    }

    Ok(())
}
