use bytes::Bytes;
use mini_redis::Command::{Get, Set};
use mini_redis::{Command, Connection, Frame};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;
const NUM_SHARDS: usize = 10;

fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379")
        .await
        .unwrap();
    println!("Listening");
    
    let db = new_sharded_db(NUM_SHARDS);
    loop {
        // The second item contains the IP and port of the new connection.
        // (TcpStream, SocketAddr)
        let (socket, _) = listener.accept().await.unwrap();

        let db = db.clone();

        println!("Accepted connection");
        // A new task is spawned for each inbound socket. The socket is
        // moved to the new task and processed there.
        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}

fn hash<T: Hash>(key: &T) -> usize {
    let mut s = DefaultHasher::new();
    key.hash(&mut s);
    s.finish() as usize
}

async fn process(socket: TcpStream, db: ShardedDb) {
    // The `Connection` lets us read/write redis **frames** instead of
    // byte streams. The `Connection` type is defined by mini-redis.
    let mut connection = Connection::new(socket);
    
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                let mut shard = db[hash(&cmd.key().to_string()) % db.len()].lock().unwrap();
                shard.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let shard = db[hash(&cmd.key().to_string()) % db.len()].lock().unwrap();
                if let Some(value) = shard.get(cmd.key()) {
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented command: {:?}", cmd),
        };

        // Write the response to the client
        connection.write_frame(&response).await.unwrap();
    }
}