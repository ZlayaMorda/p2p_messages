use crate::errors::NodeError;
use crate::errors::NodeError::{
    InvalidIpV4, ItselfConnectionError, PeriodValueError, TcpClosedError, TcpWriteError,
};
use crate::message::Message64;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::time;
use tokio::time::Interval;

const CONNECTION_TIMEOUT: u64 = 10;

/// NodeBuilder provides more flexible creation of Node with different input data
pub struct NodeBuilder {
    address: String,
    port: u16,
    period: Mutex<u64>,
    connections: Mutex<HashMap<String, Option<String>>>,
}

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            address: String::new(),
            port: 8080_u16,
            period: Mutex::new(5_u64),
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub fn address(mut self, address: String) -> NodeBuilder {
        self.address = address;
        self
    }

    pub fn port(mut self, port: u16) -> NodeBuilder {
        self.port = port;
        self
    }

    pub fn period(self, period: u64) -> Result<NodeBuilder, NodeError> {
        if period == 0 {
            return Err(PeriodValueError);
        }
        *self.period.lock().expect("Error while lock period") = period;
        Ok(self)
    }

    pub fn add_connection(
        self,
        address: String,
        connected_to_address: Option<String>,
    ) -> NodeBuilder {
        self.connections
            .lock()
            .expect("Error while lock connections")
            .insert(address, connected_to_address);
        self
    }

    pub fn build(self) -> Node {
        Node {
            address: self.address,
            port: self.port,
            period: self.period,
            connections: self.connections,
        }
    }
}

#[derive(Debug)]
pub struct Node {
    address: String,
    port: u16,
    period: Mutex<u64>,
    /// Hash map with connected addresses, if key is local listen socket value would be None,
    /// if socket address do not listen value would be Some(listen_socket)
    connections: Mutex<HashMap<String, Option<String>>>, // TODO handling connections, may be using ids
                                                         // TODO handling messages
}

impl Node {
    pub async fn bind_address(&self) -> Result<TcpListener, NodeError> {
        Ok(TcpListener::bind(format!("{}:{}", self.address, self.port)).await?)
    }

    pub async fn connect_to(self: Arc<Self>, address_to: Option<String>) -> Result<(), NodeError> {
        if let Some(address_to) = address_to {
            let local_socket: String = format!("{}:{}", self.address, self.port);
            if local_socket == address_to {
                return Err(ItselfConnectionError);
            }
            let stream: TcpStream = TcpStream::connect(&address_to).await?;
            tracing::debug!("connected to {}", address_to);
            if let Err(error) = stream.try_write(local_socket.as_bytes()) {
                tracing::warn!("Error while try write local socket {error}");
                return Err(TcpWriteError(error));
            }
            self.connections
                .lock()
                .expect("Error while lock connections")
                .insert(address_to.clone(), None);
            tokio::spawn(self.clone()._handle_thread(stream));
        }
        Ok(())
    }

    pub async fn listen_connections(
        self: Arc<Self>,
        listener: &TcpListener,
    ) -> Result<(), NodeError> {
        'listen: loop {
            let (mut stream, socket_address) = listener.accept().await?;
            tracing::debug!("connected new client {socket_address}");
            let mut buf: Vec<u8> = vec![0; 14];
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(CONNECTION_TIMEOUT)) => {
                    tracing::warn!("Timeout waiting socket address");
                    continue 'listen;
                }
                n = stream.read(&mut buf) => {
                    match n {
                        Ok(n) => {
                            match Message64::read_socket_address(n, &buf) {
                                Ok(socket_listen_address) => {
                                    self.connections.lock().expect("Error while lock connections").insert(
                                        socket_address.to_string(),
                                        Some(socket_listen_address)
                                    );
                                }
                                Err(TcpClosedError) => {
                                    tracing::warn!("Connection closed");
                                    continue 'listen;
                                }
                                Err(InvalidIpV4) => {
                                    tracing::warn!("Socket sent invalid Ip v4 address");
                                    continue 'listen;
                                }
                                _ => {
                                    tracing::error!("Unexpected error");
                                    continue 'listen;
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!("Error while reading message {err}");
                            continue 'listen;
                        }
                    }
                }
            }
            tokio::spawn(self.clone()._handle_thread(stream));
        }
    }

    pub(crate) async fn _handle_thread(self: Arc<Self>, stream: TcpStream) {
        let (mut reader, writer) = stream.into_split();
        let mut interval: Interval = time::interval(Duration::from_secs(
            *self.period.lock().expect("Error while lock period"),
        ));
        'handle: loop {
            tracing::debug!("Connections: {:?}", self.connections.lock().unwrap());
            let mut buf: Vec<u8> = vec![0; 2048];

            // Select to achieve concurrent reading and writing, writing with a tick period
            tokio::select!(
                _ = interval.tick() => {
                    self._handle_writing(&writer).await;
                }
                n = reader.read(&mut buf) => {
                    match n {
                        Ok(n) => {
                            match self._read_messages(n, &mut buf) {
                                Ok(_) => continue 'handle,
                                Err(TcpClosedError) => break 'handle,
                                Err(_) => continue 'handle,
                            }
                        }
                        Err(err) => {
                            tracing::warn!("Error while reading message {err}");
                            continue 'handle;
                        }
                    }

                }
            )
        }
    }

    pub(crate) async fn _handle_writing(&self, writer: &OwnedWriteHalf) {
        if let Err(error) =
            writer.try_write(&Message64::create_random_message(&self.address, self.port))
        {
            tracing::warn!("Error while try to write {error}");
        }
    }

    fn _read_messages(&self, n: usize, buf: &mut Vec<u8>) -> Result<(), NodeError> {
        tracing::debug!("Reading message of size {n}");
        if n == 0 {
            tracing::warn!("Connection closed");
            return Err(TcpClosedError);
        }

        Message64::read_messages(buf);
        Ok(())
    }
}
