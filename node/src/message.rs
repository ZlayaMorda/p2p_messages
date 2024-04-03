use chrono::Utc;

/// implemented only for a 64-bit memory systems
pub struct Message64 {}

impl Message64 {
    pub(crate) fn create_random_message(address: &str, port: u16) -> Vec<u8> {
        tracing::debug!("Creating message to send");
        let str_message = format!(
            "{} - Message from {}:{}",
            Utc::now().timestamp(),
            address,
            port
        );
        let message: &[u8] = str_message.as_bytes();
        let length: [u8; 8] = message.len().to_ne_bytes();
        [&length, message].concat()
    }

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

    fn _handle_random_message(message: &str) {
        println!("{}", message);
    }
}
