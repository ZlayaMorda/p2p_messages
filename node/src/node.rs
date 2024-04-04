use crate::errors::NodeError;
use crate::errors::NodeError::{ItselfConnectionError, PeriodValueError, TcpClosedError};
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
    connections: Mutex<HashMap<String, Option<String>>>, // TODO handling connections, may be using ids
                                                         // TODO handling messages
}

impl Node {
    pub async fn bind_address(&self) -> Result<TcpListener, NodeError> {
        Ok(TcpListener::bind(format!("{}:{}", self.address, self.port)).await?)
    }

    pub async fn connect_to(self: Arc<Self>, address_to: Option<String>) -> Result<(), NodeError> {
        if let Some(address_to) = address_to {
            if format!("{}:{}", self.address, self.port) == address_to {
                return Err(ItselfConnectionError);
            }
            let stream: TcpStream = TcpStream::connect(&address_to).await?;
            tracing::debug!("connected to {}", address_to);
            self.connections
                .lock()
                .expect("Error while lock connections")
                .insert(address_to.clone(), None);
            let _ = tokio::spawn(self.clone()._handle_thread(stream)).await;
        }
        Ok(())
    }

    pub async fn listen_connections(
        self: Arc<Self>,
        listener: &TcpListener,
    ) -> Result<(), NodeError> {
        loop {
            let (stream, socket_address) = listener.accept().await?;
            tracing::debug!("connected new client {socket_address}");
            tokio::spawn(self.clone()._handle_thread(stream));
        }
    }

    pub(crate) async fn _handle_thread(self: Arc<Self>, stream: TcpStream) {
        let (mut reader, writer) = stream.into_split();
        let mut interval: Interval = time::interval(Duration::from_secs(
            *self.period.lock().expect("Error while lock period"),
        ));
        loop {
            tracing::debug!("Connections: {:?}", self.connections.lock().unwrap());
            let mut buf: Vec<u8> = vec![0; 2048];

            // Select to achieve concurrent reading and writing, writing with a tick period
            tokio::select!(
                _ = interval.tick() => {
                    self._handle_writing(&writer).await;
                }
                n = reader.read(&mut buf) => {
                    match self._read_messages(n.unwrap(), &mut buf) {
                        Ok(_) => continue,
                        Err(TcpClosedError) => break,
                        Err(_) => continue,
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
