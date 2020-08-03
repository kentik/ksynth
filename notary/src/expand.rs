use std::io::{Error, Write};
use hmac_sha512::Hash;
use zstd::stream::write::Decoder;

pub struct Expand<W: Sized> {
    dec:  Decoder<Vec<u8>>,
    sink: W,
    hash: Hash,
}

impl<W: Write + Sized> Expand<W> {
    pub fn new(sink: W) -> Result<Self, Error> {
        Ok(Self {
            dec:  Decoder::new(Vec::new())?,
            sink: sink,
            hash: Hash::new(),
        })
    }

    pub fn update(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.dec.write_all(bytes)?;

        let chunk = self.dec.get_mut();
        self.hash.update(&chunk);
        self.sink.write_all(&chunk)?;

        chunk.clear();

        Ok(())
    }

    pub fn finish(mut self) -> Result<[u8; 64], Error> {
        self.dec.flush()?;

        let chunk = self.dec.into_inner();
        self.hash.update(&chunk);
        self.sink.write_all(&chunk)?;

        Ok(self.hash.finalize())
    }
}
