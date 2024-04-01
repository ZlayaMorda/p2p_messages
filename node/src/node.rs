use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Interval;
use tokio::{sync::Mutex, time};

/// NodeBuilder provides more flexible creation of Node with different input data
pub struct NodeBuilder {
    address: Arc<Mutex<String>>,
    port: Arc<Mutex<String>>,
    interval: Arc<Mutex<Interval>>,
    connections: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl NodeBuilder {
    pub fn new(period: u64) -> NodeBuilder {
        NodeBuilder {
            address: Arc::new(Mutex::new(String::new())),
            port: Arc::new(Mutex::new(String::new())),
            interval: Arc::new(Mutex::new(time::interval(Duration::from_secs(period)))),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn address(self, address: String) -> NodeBuilder {
        *self.address.lock().await = address;
        self
    }

    pub async fn port(self, port: String) -> NodeBuilder {
        *self.port.lock().await = port;
        self
    }

    pub async fn add_connection(
        self,
        address: String,
        connected_to_address: HashSet<String>,
    ) -> NodeBuilder {
        self.connections
            .lock()
            .await
            .insert(address, connected_to_address);
        self
    }

    pub async fn build(self) -> Node {
        Node {
            address: self.address,
            port: self.port,
            interval: self.interval,
            connections: self.connections,
        }
    }
}

pub struct Node {
    address: Arc<Mutex<String>>,
    port: Arc<Mutex<String>>,
    interval: Arc<Mutex<Interval>>,
    connections: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}
