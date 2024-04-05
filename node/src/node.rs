use crate::errors::NodeError;
use crate::errors::NodeError::{
    InvalidIpV4, ItselfConnectionError, PeriodValueError, TcpClosedError, TcpWriteError,
    UnexpectedMode,
};
use crate::message::Message64;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::time;
use tokio::time::Interval;

/// NodeBuilder provides more flexible creation of Node with different input data
pub struct NodeBuilder {
    address: String,
    port: u16,
    connection_timeout: u64,
    period: Mutex<u64>,
    connections: Mutex<HashMap<String, Option<String>>>,
}

impl NodeBuilder {
    pub fn new() -> NodeBuilder {
        NodeBuilder {
            address: String::new(),
            port: 8080_u16,
            connection_timeout: 10_u64,
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

    pub fn connection_timeout(mut self, timeout: u64) -> NodeBuilder {
        self.connection_timeout = timeout;
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
            connection_timeout: self.connection_timeout,
            period: self.period,
            connections: self.connections,
        }
    }
}

/// Peer struct, handle peer data and connection logic
/// Implemented for IPv4 and 64-bit memory systems
#[derive(Debug)]
pub struct Node {
    /// Node listen address
    address: String,
    /// Node listen port
    port: u16,
    /// Time wait for connection data
    connection_timeout: u64,
    /// Message sending period
    period: Mutex<u64>,
    /// Hash map with connected addresses, if key is local listen socket value would be None,
    /// if socket address do not listen value would be Some(listen_socket)
    connections: Mutex<HashMap<String, Option<String>>>,
}

impl Node {
    /// Bind address listen to
    pub async fn bind_address(&self) -> Result<TcpListener, NodeError> {
        Ok(TcpListener::bind(format!("{}:{}", self.address, self.port)).await?)
    }

    /// Connect to address and get other addresses and connect to
    /// Spawn tokio tasks for each connection and read/write in async mode
    pub async fn connect_to(self: Arc<Self>, address_to: Option<String>) -> Result<(), NodeError> {
        if let Some(address_to) = address_to {
            let local_socket: String = format!("{}:{}", self.address, self.port);
            if local_socket == address_to {
                return Err(ItselfConnectionError);
            }
            let mut stream: TcpStream = TcpStream::connect(&address_to).await?;
            tracing::debug!("connected to {}", address_to);
            self.try_write(
                &stream,
                &Message64::create_connect_message(&local_socket, 0_u8),
            )?;

            self.read_and_store_connections(&mut stream).await?;

            for (socket, _) in self.get_connections().iter() {
                let stream_connection: TcpStream = match TcpStream::connect(&socket).await {
                    Ok(stream) => stream,
                    Err(error) => {
                        tracing::warn!("Error while connect to socket {error}");
                        continue;
                    }
                };

                if let Err(err) = self.try_write(
                    &stream_connection,
                    &Message64::create_connect_message(&local_socket, 1_u8),
                ) {
                    self.get_connections().remove(socket);
                    tracing::warn!("Error while write to connect {err}");
                    continue;
                }
                tokio::spawn(Arc::clone(&self)._handle_thread(stream_connection));
            }
            self.get_connections().insert(address_to.clone(), None);
            tokio::spawn(Arc::clone(&self).clone()._handle_thread(stream));
        }
        Ok(())
    }

    /// Listen connections and share addresses with newcomers
    /// Spawn tokio tasks for each connection and read/write in async mode
    pub async fn listen_connections(
        self: Arc<Self>,
        listener: &TcpListener,
    ) -> Result<(), NodeError> {
        'listen: loop {
            let (mut stream, socket_address) = listener.accept().await?;
            tracing::debug!("connected new client {socket_address}");
            let mut buf: Vec<u8> = vec![0; 15];
            // select to achieve concurrent sleeping and reading
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(self.connection_timeout)) => {
                    tracing::warn!("Timeout waiting socket address");
                    continue 'listen;
                }
                n = stream.read(&mut buf) => {
                    match n {
                        Ok(n) => {
                            if let Err(_) = self.read_and_share_connections(&stream, &socket_address, n, &buf).await {
                                continue 'listen;
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

    /// Handle connection
    /// Write with node period timeout
    /// Read messages
    pub(crate) async fn _handle_thread(self: Arc<Self>, stream: TcpStream) {
        let (mut reader, writer) = stream.into_split();
        let mut interval: Interval = time::interval(Duration::from_secs(*self.get_period()));
        'handle: loop {
            tracing::debug!("Connections: {:?}", self.get_connections());
            let mut buf: Vec<u8> = vec![0; 2048];
            // Select to achieve concurrent reading and writing, writing with a tick period
            tokio::select!(
                _ = interval.tick() => {
                    self.handle_writing(&writer, &Message64::create_random_message(&self.address, self.port)).await;
                }
                n = reader.read(&mut buf) => {
                    match n {
                        Ok(n) => {
                            match self.read_messages(n, &mut buf) {
                                Ok(_) => continue 'handle,
                                Err(TcpClosedError) => {
                                    self.get_connections().remove(&reader.peer_addr().expect("Could not get peer addr").to_string());
                                    break 'handle
                                }
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

    pub(crate) async fn handle_writing(&self, writer: &OwnedWriteHalf, message: &Vec<u8>) {
        if let Err(error) = writer.try_write(message) {
            tracing::warn!("Error while try to write {error}");
        }
    }

    fn read_messages(&self, n: usize, buf: &mut Vec<u8>) -> Result<(), NodeError> {
        tracing::debug!("Reading message of size {n}");
        if n == 0 {
            tracing::warn!("Connection closed");
            return Err(TcpClosedError);
        }

        Message64::read_messages(buf);
        Ok(())
    }

    fn try_write(&self, stream: &TcpStream, message: &[u8]) -> Result<(), NodeError> {
        if let Err(error) = stream.try_write(message) {
            tracing::warn!("Error while try write {error}");
            return Err(TcpWriteError(error));
        }
        Ok(())
    }

    /// Mutex handler
    fn get_connections(&self) -> MutexGuard<HashMap<String, Option<String>>> {
        match self.connections.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let guard = poisoned.into_inner();
                tracing::error!("Connections poisoned");
                guard
            }
        }
    }

    /// Mutex handler
    fn get_period(&self) -> MutexGuard<u64> {
        match self.period.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let guard = poisoned.into_inner();
                tracing::error!("Connections poisoned");
                guard
            }
        }
    }

    /// Read shared connections and store to HashMap
    /// Read more about modes in [Message64]
    async fn read_and_store_connections(&self, stream: &mut TcpStream) -> Result<(), NodeError> {
        let mut buf: Vec<u8> = vec![0; 15];
        loop {
            match stream.read(&mut buf).await {
                Ok(n) => match Message64::read_socket_address(n, &buf) {
                    Ok((mode, message)) => match mode {
                        1_u8 => {
                            // Just connect
                            self.get_connections().insert(message, None);
                            continue;
                        }
                        2_u8 => {
                            // Last one to just connect
                            self.get_connections().insert(message, None);
                            break;
                        }
                        3_u8 => {
                            // Empty connections
                            break;
                        }
                        num => {
                            tracing::error!("Unexpected mode number {num}");
                            continue;
                        }
                    },
                    Err(err) => return Err(err),
                },
                Err(err) => {
                    tracing::warn!("Error while reading message {err}");
                    continue;
                }
            }
        }
        Ok(())
    }

    /// Read message about new connection, depending on the mode process it
    /// Share own connections
    /// Read more about modes in [Message64]
    async fn read_and_share_connections(
        &self,
        stream: &TcpStream,
        socket_address: &SocketAddr,
        n: usize,
        buf: &Vec<u8>,
    ) -> Result<(), NodeError> {
        match Message64::read_socket_address(n, &buf) {
            Ok((mode, message)) => {
                match mode {
                    0_u8 => {
                        self.share_connections(&stream).await?;
                    }
                    1_u8 => {} // Just connect and continue
                    num => {
                        tracing::error!("Unexpected mode number {num}");
                        return Err(UnexpectedMode);
                    }
                }
                self.get_connections()
                    .insert(socket_address.to_string(), Some(message));
                Ok(())
            }
            Err(TcpClosedError) => Err(TcpClosedError),
            Err(InvalidIpV4) => Err(InvalidIpV4),
            Err(err) => {
                tracing::error!("Unexpected error");
                Err(err)
            }
        }
    }

    async fn share_connections(&self, stream: &TcpStream) -> Result<(), NodeError> {
        let connections = self.get_connections();
        if connections.len() == 0 {
            self.try_write(
                &stream,
                &Message64::create_connect_message("127.0.0.1:0000", 3_u8), // empty mode
            )?;
        } else {
            let mut iter = connections.iter().peekable();
            let mut mode: u8 = 1_u8; // connection mode
            let mut message: Vec<u8>;
            while let Some(item) = iter.next() {
                if iter.peek().is_none() {
                    mode = 2_u8; // end mode
                }
                if let Some(value) = item.1 {
                    message = Message64::create_connect_message(value, mode);
                } else {
                    message = Message64::create_connect_message(item.0, mode);
                }
                tracing::debug!("WRITE FROM LISTENER {:?}", message);
                self.try_write(&stream, &message)?
            }
        }
        Ok(())
    }
}
