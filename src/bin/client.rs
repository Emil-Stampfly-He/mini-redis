use bytes::Bytes;
use mini_redis::{client};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
enum Command {
    Get {
        key: String,
        resp: Responder<Option<Bytes>>,
    },
    Set {
        key: String,
        value: Bytes,
        resp: Responder<()>,
    },
}

type Responder<T> = oneshot::Sender<mini_redis::Result<T>>;

#[tokio::main]
async fn main() {
    // Capacity at most 32
    let (tx, mut rx) = mpsc::channel(32);
    let tx2 = tx.clone();
    
    // Manager task
    let manager = tokio::spawn(async move {
        // Establish a connection to the server
        let mut client = client::connect("localhost:6379").await.unwrap();
        
        // Start receiving messages
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Command::Get { key, resp } => {
                    let result = client.get(&key).await;
                    let _ = resp.send(result);
                }
                Command::Set { key, value, resp} => {
                    let result = client.set(&key, value).await;
                    let _ = resp.send(result);
                }
            }
        }
    });

    let t1 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Get {
            key: "foo".to_string(),
            resp: resp_tx,
        };
        
        // Send the GET request
        tx.send(cmd).await.unwrap();
        
        // Await the response
        let resp = resp_rx.await.unwrap();
        println!("GOT = {:?}", resp);
    });

    let t2 = tokio::spawn(async move {
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = Command::Set {
            key: "foo".to_string(),
            value: "bar".into(),
            resp: resp_tx,
        };
        
        // Send the SET request
        tx2.send(cmd).await.unwrap();
        
        // Await the response
        let resp = resp_rx.await.unwrap();
        println!("GOT = {:?}", resp);
    });
    
    t1.await.unwrap();
    t2.await.unwrap();
    manager.await.unwrap();
}