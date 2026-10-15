use io::{aedat4_decoder::Aedat4, codec::DecoderFactory as _};
use std::io::Error;
use tokio::fs::File;

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    // TODO: REMOVE
    let file = File::open("/home/david/Downloads/davis346.aedat4").await?;

    let decoder = Aedat4::open(&Aedat4, file).await.map_err(|err| {
        Error::other(
            format!("error opening file: {err:?}"),
        )
    })?;

    println!("packet table: {:?}", decoder.file_data_table.get());

    Ok(())
}
