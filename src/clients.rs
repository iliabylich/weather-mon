use std::collections::HashMap;
use tokio::{
    io::AsyncWriteExt,
    net::{
        UnixStream,
        unix::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use tokio_stream::{StreamMap, StreamNotifyClose};
use tokio_util::codec::{BytesCodec, FramedRead};

pub struct Clients {
    readers: StreamMap<u64, StreamNotifyClose<FramedRead<OwnedReadHalf, BytesCodec>>>,
    writers: HashMap<u64, OwnedWriteHalf>,
}

impl Clients {
    pub(crate) fn new() -> Self {
        Self {
            readers: StreamMap::new(),
            writers: HashMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, id: u64, client: UnixStream) {
        let (reader, writer) = client.into_split();
        let client = StreamNotifyClose::new(FramedRead::new(reader, BytesCodec::new()));
        self.readers.insert(id, client);
        self.writers.insert(id, writer);
    }

    pub(crate) async fn send(&mut self, id: u64, buf: &[u8]) {
        let Some(writer) = self.writers.get_mut(&id) else {
            return;
        };

        if let Err(err) = writer.write_all(buf).await {
            log::error!("client {id} failed to write: {err:?}");
            self.readers.remove(&id);
            self.writers.remove(&id);
        }
    }

    pub(crate) async fn broadcast(&mut self, buf: &[u8]) {
        let mut to_drop = vec![];

        for (id, writer) in &mut self.writers {
            if let Err(err) = writer.write_all(buf).await {
                log::error!("client {id} failed to write: {err:?}");
                to_drop.push(*id);
                return;
            }
        }

        for id in to_drop {
            self.readers.remove(&id);
            self.writers.remove(&id);
        }
    }
}
