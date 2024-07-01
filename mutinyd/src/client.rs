use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{oneshot, mpsc};
use tokio::net::UnixStream;
use serde::Serialize;
use rmp_serde::Serializer;
use error_set::error_set;

use crate::protocol::{Request, Response};

pub struct ClientRequest {
    pub request: Request,
    pub response: oneshot::Sender<Response>,
}

pub struct Client<Reader: AsyncBufRead + AsyncReadExt + Unpin, Writer: AsyncWrite + AsyncWriteExt + Unpin> {
    request_sender: mpsc::Sender<ClientRequest>,
    reader: Reader,
    writer: Writer,
}

error_set! {
    ClientError = {
        IoError(std::io::Error),
        DecodeRequestError(rmp_serde::decode::Error),
        EncodeResponseError(rmp_serde::encode::Error),
        ReadLengthError(std::num::TryFromIntError),
        ReadResponseError(oneshot::error::RecvError),
        SendRequestError(mpsc::error::SendError<ClientRequest>),
    };
}

impl<Reader: AsyncBufRead + AsyncReadExt + Unpin, Writer: AsyncWrite + AsyncWriteExt + Unpin> Client<Reader, Writer> {
    async fn read_request(&mut self) -> Result<Request, ClientError> {
        let length = self.reader.read_u32().await?;
        let mut buf = vec![0; length as usize];
        self.reader.read_exact(&mut buf).await?;
        Ok(rmp_serde::from_slice::<Request>(&buf)?)
    }

    async fn write_response(&mut self, response: Response) -> Result<(), ClientError> {
        // The rmp_serde::Serializer is not async and can not write
        // directly to an AsyncWrite, write to a buffer first.
        let mut serialized = Vec::<u8>::new();
        response.serialize(&mut Serializer::new(&mut serialized).with_struct_map())?;
        let len = u32::try_from(serialized.len())?;
        self.writer.write_all(&len.to_be_bytes()).await?;
        self.writer.write_all(&serialized).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn handle_next_request(&mut self) -> Result<(), ClientError> {
        let request = self.read_request().await?;
        let (tx, rx) = oneshot::channel();
        self.request_sender.send(ClientRequest {
            request: request,
            response: tx,
        }).await?;
        let response = rx.await?;
        self.write_response(response).await?;
        Ok(())
    }

    pub async fn start(mut self) -> () {
        loop {
            if let Err(err) = self.handle_next_request().await {
                if let ClientError::IoError(e) = err {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        // Client disconnected
                        break
                    }
                    panic!("{:?}", e);
                } else {
                    self.write_response(Response::Error {
                        message: format!("{:?}", err),
                    }).await.expect("Send error response");
                }
            }
        }
    }
}

pub fn create_client(
    stream: UnixStream,
    request_sender: mpsc::Sender<ClientRequest>,
) -> Client<BufReader<tokio::net::unix::OwnedReadHalf>, tokio::net::unix::OwnedWriteHalf> {
    let (reader, writer) = stream.into_split();
    Client {
        request_sender,
        reader: BufReader::new(reader),
        writer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn decode_request_and_send_over_channel() {
        let response = Vec::new();
        let (tx, mut rx) = mpsc::channel(100);
        let handle = tokio::spawn(async move {
            let mut request = Vec::new();

            // Serialize request
            let mut serialized = Vec::<u8>::new();
            Request::LocalPeerId.serialize(
                &mut Serializer::new(&mut serialized).with_struct_map()
            ).unwrap();
            let len = u32::try_from(serialized.len()).unwrap();
            // Write message length
            request.write_all(&len.to_be_bytes()).await.unwrap();
            // Write serialized message
            request.write_all(&serialized).await.unwrap();

            let client = Client {
                request_sender: tx,
                reader: request.as_slice(),
                writer: response,
            };
            client.start().await;
        });
        let message = rx.recv().await.unwrap();
        handle.abort();
        assert_eq!(message.request, Request::LocalPeerId);
    }

    #[tokio::test]
    async fn receive_from_channel_and_write_serialized_response() {
        // Use temporary file so we can read it later without having to
        // manage ownership over a vector.
        let path = std::env::temp_dir().join("mutiny-response.tmp");
        let response = tokio::fs::File::create(&path).await.unwrap();

        let (tx, mut rx) = mpsc::channel(100);
        let handle = tokio::spawn(async move {
            let mut request = Vec::new();

            // Serialize request
            let mut serialized = Vec::<u8>::new();
            Request::LocalPeerId.serialize(
                &mut Serializer::new(&mut serialized).with_struct_map()
            ).unwrap();
            let len = u32::try_from(serialized.len()).unwrap();
            // Write message length
            request.write_all(&len.to_be_bytes()).await.unwrap();
            // Write serialized message
            request.write_all(&serialized).await.unwrap();

            let client = Client {
                request_sender: tx,
                reader: request.as_slice(),
                writer: response,
            };
            client.start().await;
        });

        let message = rx.recv().await.unwrap();
        assert_eq!(message.request, Request::LocalPeerId);
        let res = Response::LocalPeerId {
            peer_id: String::from("peer123")
        };
        // Serialize response
        let mut serialized = Vec::<u8>::new();
        res.serialize(
            &mut Serializer::new(&mut serialized).with_struct_map()
        ).unwrap();
        let len = u32::try_from(serialized.len()).unwrap();
        let mut expected = Vec::new();
        // Write message length
        expected.write_all(&len.to_be_bytes()).await.unwrap();
        // Write serialized message
        expected.write_all(&serialized).await.unwrap();
        // Send to client task
        message.response.send(res).unwrap();

        // Give the client task chance to write the response
        tokio::time::sleep(std::time::Duration::from_millis(0)).await;
        assert_eq!(tokio::fs::read(&path).await.unwrap(), expected);

        handle.abort();
        tokio::fs::remove_file(&path).await.unwrap();
    }
}
