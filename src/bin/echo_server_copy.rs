use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = TcpStream::connect("localhost:6378").await?;
    let (mut reader, mut writer) = io::split(socket);
    
    tokio::spawn(async move { 
        writer.write_all(b"hello\r\n").await?;
        writer.write_all(b"world\r\n").await?;
        Ok::<_, io::Error>(())
    });
    
    let mut buf = vec![0; 128];
    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        println!("GOT {:?}", &buf[..n])
    }
    
    Ok(())
}
