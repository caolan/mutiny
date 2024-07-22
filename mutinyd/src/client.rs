use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tokio::net::UnixStream;
use serde::Serialize;
use rmp_serde::Serializer;
use error_set::error_set;

use crate::protocol::{Request, RequestBody, Response, ResponseBody};

pub struct ClientRequest {
    pub request: RequestBody,
    pub response: mpsc::Sender<ResponseBody>,
}

struct RequestHandler {
    pub request_sender: mpsc::Sender<ClientRequest>,
    pub responses_tx: mpsc::Sender<Response>,
}

impl RequestHandler {
    async fn start(self, request: Request) -> () {
        let (tx, mut rx) = mpsc::channel(100);
        let request_id = request.id;
        let result = self.request_sender.send(ClientRequest {
            request: request.body,
            response: tx,
        }).await;
        if let Err(err) = result {
            eprintln!("Error sending client request for handling: {}", err);
            return;
        }
        while let Some(body) = rx.recv().await {
            if let Err(err) = self.responses_tx.send(Response {request_id, body}).await {
                eprintln!("Error queuing response for client: {}", err);
                return;
            }
        }
    }
}

pub struct Client<Reader: AsyncReadExt + Unpin, Writer: AsyncWrite + AsyncWriteExt + Unpin> {
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
        SendRequestError(mpsc::error::SendError<ClientRequest>),
    };
}

impl<Reader: AsyncReadExt + Unpin, Writer: AsyncWrite + AsyncWriteExt + Unpin> Client<Reader, Writer> {
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

    fn spawn_request_handler(&mut self, request: Request, responses_tx: mpsc::Sender<Response>) {
        let handler = RequestHandler {
            request_sender: self.request_sender.clone(),
            responses_tx: responses_tx.clone(),
        };
        tokio::spawn(handler.start(request));
    }

    // TODO: split into separate read requests / write response loops
    pub async fn start(mut self) -> () {
        let (responses_tx, mut responses_rx) = mpsc::channel(100);
        loop {
            tokio::select! {
                req = self.read_request() => {
                    match req {
                        Ok(request) => {
                            self.spawn_request_handler(request, responses_tx.clone());
                        },
                        Err(err) => {
                            if let ClientError::IoError(e) = err {
                                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                    // Client disconnected
                                    return;
                                }
                            } else {
                                // Invalid request, disconnect client
                                eprintln!("Invalid request: {}", err);
                                if let Err(e) = self.writer.shutdown().await {
                                    eprintln!("Failed to shutdown client writer: {}", e);
                                }
                                return;
                            }
                        },
                    }
                },
                res = responses_rx.recv() => {
                    match res {
                        Some(response) => {
                            if let Err(err) = self.write_response(response).await {
                                if let ClientError::IoError(e) = err {
                                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                        // Client disconnected
                                        return;
                                    }
                                } else {
                                    eprintln!("Error sending response: {}", err);
                                    if let Err(e) = self.writer.shutdown().await {
                                        eprintln!("Failed to shutdown client writer: {}", e);
                                    }
                                    return;
                                }
                            }
                        },
                        None => break,
                    }
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
    return Client {
        request_sender,
        reader: BufReader::new(reader),
        writer,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::RequestBody;
    use tokio::time::{timeout, sleep, Duration};

    #[tokio::test]
    async fn decode_request_and_send_over_channel() {
        let response = Vec::new();
        let (tx, mut rx) = mpsc::channel(100);
        let handle = tokio::spawn(async move {
            let mut request = Vec::new();

            // Serialize request
            let mut serialized = Vec::<u8>::new();
            Request { id: 1, body: RequestBody::LocalPeerId }.serialize(
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
        assert_eq!(message.request, RequestBody::LocalPeerId);
    }

    #[tokio::test]
    async fn receive_from_channel_and_write_serialized_response() {
        let (response_writer, mut response_reader) = tokio::io::duplex(64);
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            let (mut request_writer, request_reader) = tokio::io::duplex(64);

            // Serialize request
            let mut serialized = Vec::<u8>::new();
            Request {id: 2, body: RequestBody::LocalPeerId }.serialize(
                &mut Serializer::new(&mut serialized).with_struct_map()
            ).unwrap();
            let len = u32::try_from(serialized.len()).unwrap();
            // Write message length
            request_writer.write_all(&len.to_be_bytes()).await.unwrap();
            // Write serialized message
            request_writer.write_all(&serialized).await.unwrap();

            let client = Client {
                request_sender: tx,
                reader: request_reader,
                writer: response_writer,
            };
            client.start().await;
        });

        let message = rx.recv().await.unwrap();
        assert_eq!(message.request, RequestBody::LocalPeerId);
        let res = Response {
            request_id: 2,
            body: ResponseBody::LocalPeerId {
                peer_id: String::from("peer123")
            },
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
        message.response.send(res.body).await.unwrap();

        // Read the response
        let mut actual = vec![0; expected.len()];
        timeout(
            Duration::from_millis(1000),
            response_reader.read_exact(&mut actual)
        ).await.unwrap().unwrap();
        assert_eq!(actual, expected);

        handle.abort();
    }

    #[tokio::test]
    async fn handle_concurrent_requests() {
        let (response_writer, mut response_reader) = tokio::io::duplex(64);
        let (tx, mut rx) = mpsc::channel(100);

        let handle = tokio::spawn(async move {
            let (mut request_writer, request_reader) = tokio::io::duplex(64);

            tokio::spawn(async move {
                // Write multiple requests without waiting for a response
                let to_send = [
                    Request {id: 1, body: RequestBody::LocalPeerId },
                    Request {id: 2, body: RequestBody::Peers },
                ];
                for msg in to_send {
                    // Serialize request
                    let mut serialized = Vec::<u8>::new();
                    msg.serialize(
                        &mut Serializer::new(&mut serialized).with_struct_map()
                    ).unwrap();
                    let len = u32::try_from(serialized.len()).unwrap();
                    // Write message length
                    request_writer.write_all(&len.to_be_bytes()).await.unwrap();
                    // Write serialized message
                    request_writer.write_all(&serialized).await.unwrap();
                    // Delay before sending next message
                    sleep(Duration::from_millis(100)).await;
                }
            });

            let client = Client {
                request_sender: tx,
                reader: request_reader,
                writer: response_writer,
            };
            client.start().await;
        });

        // Read first request
        let message1 = timeout(Duration::from_millis(1000), rx.recv()).await.unwrap().unwrap();
        assert_eq!(message1.request, RequestBody::LocalPeerId);

        // Read second request
        let message2 = timeout(Duration::from_millis(1000), rx.recv()).await.unwrap().unwrap();
        assert_eq!(message2.request, RequestBody::Peers);

        // Vector to hold expected serialized response data
        let mut expected = Vec::new();

        // Respond to second request
        {
            let res = Response {
                request_id: 2,
                body: ResponseBody::Peers {
                    peers: vec![String::from("peer2")],
                },
            };
            // Serialize response
            let mut serialized = Vec::<u8>::new();
            res.serialize(
                &mut Serializer::new(&mut serialized).with_struct_map()
            ).unwrap();
            let len = u32::try_from(serialized.len()).unwrap();
            // Write message length
            expected.write_all(&len.to_be_bytes()).await.unwrap();
            // Write serialized message
            expected.write_all(&serialized).await.unwrap();
            // Send to client task
            message2.response.send(res.body).await.unwrap();
        }
        // Respond to first request
        {
            let res = Response {
                request_id: 1,
                body: ResponseBody::LocalPeerId {
                    peer_id: String::from("peer1"),
                },
            };
            // Serialize response
            let mut serialized = Vec::<u8>::new();
            res.serialize(
                &mut Serializer::new(&mut serialized).with_struct_map()
            ).unwrap();
            let len = u32::try_from(serialized.len()).unwrap();
            // Write message length
            expected.write_all(&len.to_be_bytes()).await.unwrap();
            // Write serialized message
            expected.write_all(&serialized).await.unwrap();
            // Send to client task
            message1.response.send(res.body).await.unwrap();
        }

        // Read the responses
        let mut actual = vec![0; expected.len()];
        timeout(
            Duration::from_millis(1000),
            response_reader.read_exact(&mut actual)
        ).await.unwrap().unwrap();
        assert_eq!(actual, expected);

        handle.abort();
    }
}
