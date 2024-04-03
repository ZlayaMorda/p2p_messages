use crate::errors::NodeError;
use crate::errors::NodeError::{ItselfConnectionError, PeriodValueError, TcpClosedError};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;
use tokio::time::Interval;

/// NodeBuilder provides more flexible creation of Node with different input data
pub struct NodeBuilder {
    address: String,
    port: u16,
    period: Mutex<u64>,
    connections: Mutex<HashMap<String, HashSet<String>>>,
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
        connected_to_address: HashSet<String>,
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
    connections: Mutex<HashMap<String, HashSet<String>>>, // TODO handling connections, may be using ids
                                                          // TODO handling messages
}

impl Node {
    pub async fn bind_address(&self) -> Result<TcpListener, NodeError> {
        Ok(TcpListener::bind(format!(
            "{}:{}",
            self.address,
            self.port
        ))
        .await?)
    }

    //TODO change for logs and error handling
    pub async fn connect_to(self: Arc<Self>, address_to: Option<String>) -> Result<(), NodeError> {
        if let Some(address_to) = address_to {
            if format!(
                "{}:{}",
                self.address,
                self.port
            ) == address_to
            {
                return Err(ItselfConnectionError);
            }
            let stream = TcpStream::connect(&address_to).await?;
            tracing::info!("connected to {}", address_to);
            self._handle_thread(stream).await;
        }
        Ok(())
    }

    //TODO change for logs and error handling
    pub async fn listen_connections(
        self: Arc<Self>,
        listener: &TcpListener,
    ) -> Result<(), NodeError> {
        loop {
            let (stream, socket_address) = listener.accept().await?;
            tracing::info!("connected new client {socket_address}");
            self.clone()._handle_thread(stream).await;
        }
    }

    pub(crate) async fn _handle_thread(self: Arc<Self>, stream: TcpStream) {
        tokio::spawn(async move {
            let (reader, writer) = stream.into_split();
            let mut interval = time::interval(Duration::from_secs(
                *self.period.lock().expect("Error while lock period"),
            ));
            loop {
                self._handle_writing(&writer, &mut interval).await;
                match self._handle_reading(&reader) {
                    Ok(_) => continue,
                    Err(TcpClosedError) => break,
                    Err(_) => continue,
                }
            }
        });
    }

    //TODO change for logs and error handling
    pub(crate) fn _handle_reading(&self, reader: &OwnedReadHalf) -> Result<(), NodeError> {
        let mut buf: Vec<u8> = vec![0; 2048];
        match reader.try_read(&mut buf) {
            Ok(n) => self._read_messages(n, &mut buf),
            Err(err) => Err(NodeError::from(err)),
        }
    }

    //TODO change for logs and error handling
    pub(crate) async fn _handle_writing(&self, writer: &OwnedWriteHalf, interval: &mut Interval) {
        if let Err(error) = writer.try_write(&self._create_message().await) {
            tracing::warn!("Error while try to write {error}");
        }
        interval.tick().await;
    }

    /// implemented only for a 64-bit memory systems
    pub(crate) async fn _create_message(&self) -> Vec<u8> {
        tracing::debug!("Creating message to send");
        let str_message = format!(
            "{} - Message from {}:{}",
            Utc::now().timestamp(),
            self.address,
            self.port
        );
        let message: &[u8] = str_message.as_bytes();
        let length: [u8; 8] = message.len().to_ne_bytes();
        [&length, message].concat()
    }

    /// implemented only for a 64-bit memory systems
    pub(crate) fn _read_messages(&self, n: usize, buf: &mut Vec<u8>) -> Result<(), NodeError> {
        tracing::debug!("Reading message of size {n}");
        if n == 0 {
            // Connection closed
            tracing::warn!("Connection closed");
            return Err(TcpClosedError);
        }

        while let Some((msg_len_bytes, rest)) = buf.split_first_chunk::<8>() {
            let msg_len = usize::from_ne_bytes(*msg_len_bytes);
            if rest.len() < msg_len || msg_len == 0 {
                break;
            }

            let message = rest[..msg_len].to_vec();
            println!("{}", String::from_utf8_lossy(&message).trim().to_string());

            buf.drain(..8 + msg_len);
        }
        Ok(())
    }
}
