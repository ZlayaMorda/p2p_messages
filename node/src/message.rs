use crate::errors::NodeError;
use crate::errors::NodeError::{InvalidIpV4, TcpClosedError};
use chrono::Utc;
use regex::Regex;

/// implemented only for a 64-bit memory systems and IPv4
/// 
/// Modes:
///     0 - first connection message, should ask other peers addresses to connect
///     1 - connect to rest peers, do not need share with other peers
///     2 - last address, need to stop reading shared addresses
///     3 - do not connect to any addresses, need to stop reading shared addresses
pub struct Message64 {}

impl Message64 {
    pub(crate) fn create_random_message(address: &str, port: u16) -> Vec<u8> {
        tracing::debug!("Creating random message to send");
        let str_message: String = format!(
            "{} - Message from {}:{}",
            Utc::now().timestamp(),
            address,
            port
        );
        Self::create_message(str_message)
    }

    /// First digit in message is mode:
    ///     0 - first connection message, should ask other peers addresses to connect
    ///     1 - connect to rest peers, do not need share with other peers
    ///     2 - last address, need to stop reading shared addresses
    ///     3 - do not connect to any addresses, need to stop reading shared addresses
    pub(crate) fn create_connect_message(socket: &str, mode: u8) -> Vec<u8> {
        tracing::debug!("Creating connect message to send");
        [&[mode], socket.as_bytes()].concat()
    }

    /// Create message with len at the start to read stuck one
    pub(crate) fn create_message(str_message: String) -> Vec<u8> {
        let message: &[u8] = str_message.as_bytes();
        let length: [u8; 8] = message.len().to_ne_bytes();
        [&length, message].concat()
    }

    /// Read message len and then payload
    pub(crate) fn read_messages(buf: &mut Vec<u8>) {
        while let Some((msg_len_bytes, rest)) = buf.split_first_chunk::<8>() {
            let msg_len: usize = usize::from_ne_bytes(*msg_len_bytes);
            if rest.len() < msg_len || msg_len == 0 {
                break;
            }

            let message: String = String::from_utf8_lossy(&rest[..msg_len].to_vec())
                .trim()
                .to_string();
            Self::_handle_random_message(&message);
            buf.drain(..8 + msg_len);
        }
    }

    pub(crate) fn read_socket_address(n: usize, buf: &Vec<u8>) -> Result<(u8, String), NodeError> {
        if n == 0 {
            tracing::warn!("Connection closed");
            return Err(TcpClosedError);
        }

        let re = Regex::new(r"^(\d{1,3}\.){3}\d{1,3}:\d{1,5}$").expect("Pattern should be valid");
        let mode = buf[0];
        let message: String = String::from_utf8_lossy(&buf[1..]).trim().to_string();

        if re.is_match(&message) {
            return Ok((mode, message));
        } else {
            tracing::warn!("Socket sent invalid Ip v4 address {} - {}", mode, message);
            Err(InvalidIpV4)
        }
    }

    fn _handle_random_message(message: &str) {
        println!("{}", message);
    }
}
