use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType, decoder, encoder};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;

use crate::errors::Error;

const SEND_ADDR: &str = "127.0.0.1:11000";
const RECV_ADDR: &str = "127.0.0.1:0";
const QUERY_TIMEOUT: Duration = Duration::from_millis(1000);
const RECV_BUF_SIZE: usize = 65535;

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<OscMessage>>>>;

pub trait FromOsc: Sized {
    fn from_osc(args: &[OscType]) -> Result<Self, Error>;
}

impl FromOsc for f64 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Float(f) => return Ok(Self::from(*f)),
                OscType::Double(d) => return Ok(*d),
                _ => {}
            }
        }
        Err(Error::UnexpectedResponse(
            "expected float in OSC args".into(),
        ))
    }
}

impl FromOsc for i32 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            if let OscType::Int(i) = arg {
                return Ok(*i);
            }
        }
        Err(Error::UnexpectedResponse("expected int in OSC args".into()))
    }
}

impl FromOsc for String {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            if let OscType::String(s) = arg {
                return Ok(s.clone());
            }
        }
        Err(Error::UnexpectedResponse(
            "expected string in OSC args".into(),
        ))
    }
}

impl FromOsc for f32 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            if let OscType::Float(f) = arg {
                return Ok(*f);
            }
        }
        Err(Error::UnexpectedResponse("expected f32 in OSC args".into()))
    }
}

impl FromOsc for bool {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Bool(b) => return Ok(*b),
                OscType::Int(i) => return Ok(*i != 0),
                _ => {}
            }
        }
        Err(Error::UnexpectedResponse(
            "expected bool in OSC args".into(),
        ))
    }
}

#[must_use]
pub fn extract_strings(args: &[OscType], skip: usize) -> Vec<String> {
    args.iter()
        .skip(skip)
        .filter_map(|a| {
            if let OscType::String(s) = a {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect()
}

#[must_use]
pub fn extract_floats(args: &[OscType], skip: usize) -> Vec<f32> {
    args.iter()
        .skip(skip)
        .filter_map(|a| {
            if let OscType::Float(f) = a {
                Some(*f)
            } else {
                None
            }
        })
        .collect()
}

pub struct OscClient {
    socket: UdpSocket,
    send_addr: SocketAddr,
    pending: PendingMap,
    query_mutex: Mutex<()>,
}

impl OscClient {
    pub async fn new(cancel: CancellationToken) -> Result<Arc<Self>, Error> {
        let socket = UdpSocket::bind(RECV_ADDR).await?;
        let send_addr: SocketAddr = SEND_ADDR
            .parse()
            .map_err(|e| Error::Config(format!("invalid send address: {e}")))?;
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(Self {
            socket,
            send_addr,
            pending,
            query_mutex: Mutex::new(()),
        });

        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            let mut buf = vec![0u8; RECV_BUF_SIZE];
            loop {
                tokio::select! {
                    () = cancel.cancelled() => {
                        tracing::debug!("OSC recv task shutting down");
                        break;
                    }
                    result = client_clone.socket.recv_from(&mut buf) => {
                        match result {
                            Ok((size, _addr)) => {
                                Self::handle_packet(&buf[..size], &client_clone.pending).await;
                            }
                            Err(e) => {
                                tracing::warn!("OSC recv error: {e}");
                            }
                        }
                    }
                }
            }
        });

        Ok(client)
    }

    async fn handle_packet(data: &[u8], pending: &PendingMap) {
        let packet = match decoder::decode_udp(data) {
            Ok((_, packet)) => packet,
            Err(e) => {
                tracing::warn!("failed to decode OSC packet: {e}");
                return;
            }
        };

        match packet {
            OscPacket::Message(msg) => {
                let mut map = pending.lock().await;
                if let Some(sender) = map.remove(&msg.addr) {
                    let _ = sender.send(msg);
                } else {
                    tracing::trace!("unsolicited OSC message: {}", msg.addr);
                }
            }
            OscPacket::Bundle(_) => {
                tracing::trace!("ignoring OSC bundle");
            }
        }
    }

    pub async fn send(&self, address: &str, args: Vec<OscType>) -> Result<(), Error> {
        let msg = OscMessage {
            addr: address.to_string(),
            args,
        };
        let packet = OscPacket::Message(msg);
        let buf = encoder::encode(&packet)
            .map_err(|e| Error::OscDecode(format!("failed to encode OSC message: {e}")))?;
        self.socket.send_to(&buf, self.send_addr).await?;
        Ok(())
    }

    pub async fn query(&self, address: &str, args: Vec<OscType>) -> Result<OscMessage, Error> {
        let _guard = self.query_mutex.lock().await;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(address.to_string(), tx);

        if let Err(e) = self.send(address, args).await {
            self.pending.lock().await.remove(address);
            return Err(e);
        }

        match tokio::time::timeout(QUERY_TIMEOUT, rx).await {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_)) => {
                self.pending.lock().await.remove(address);
                Err(Error::OscDecode(
                    "OSC listener task stopped unexpectedly".into(),
                ))
            }
            Err(_) => {
                self.pending.lock().await.remove(address);
                Err(Error::OscTimeout)
            }
        }
    }

    pub async fn query_val<T: FromOsc>(
        &self,
        address: &str,
        args: Vec<OscType>,
    ) -> Result<T, Error> {
        let msg = self.query(address, args).await?;
        T::from_osc(&msg.args)
    }
}

#[cfg(test)]
mod tests {
    use rosc::OscType;

    use super::*;

    #[test]
    fn f32_from_osc_normal() {
        let args = vec![OscType::Float(120.0)];
        assert!((f32::from_osc(&args).unwrap() - 120.0).abs() < f32::EPSILON);
    }

    #[test]
    fn f64_from_osc_float() {
        let args = vec![OscType::Float(120.0)];
        assert!((f64::from_osc(&args).unwrap() - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn f64_from_osc_double() {
        let args = vec![OscType::Double(120.5)];
        assert!((f64::from_osc(&args).unwrap() - 120.5).abs() < f64::EPSILON);
    }

    #[test]
    fn string_from_osc_with_prepended_index() {
        let args = vec![OscType::Int(0), OscType::String("Bass".into())];
        assert_eq!(String::from_osc(&args).unwrap(), "Bass");
    }

    #[test]
    fn i32_from_osc_normal() {
        let args = vec![OscType::Int(42)];
        assert_eq!(i32::from_osc(&args).unwrap(), 42);
    }

    #[test]
    fn bool_from_osc_bool() {
        let args = vec![OscType::Bool(true)];
        assert!(bool::from_osc(&args).unwrap());
    }

    #[test]
    fn bool_from_osc_int_one() {
        let args = vec![OscType::Int(1)];
        assert!(bool::from_osc(&args).unwrap());
    }

    #[test]
    fn bool_from_osc_int_zero() {
        let args = vec![OscType::Int(0)];
        assert!(!bool::from_osc(&args).unwrap());
    }

    #[test]
    fn all_from_osc_empty_args_err() {
        let args: Vec<OscType> = vec![];
        assert!(f32::from_osc(&args).is_err());
        assert!(f64::from_osc(&args).is_err());
        assert!(i32::from_osc(&args).is_err());
        assert!(String::from_osc(&args).is_err());
        assert!(bool::from_osc(&args).is_err());
    }

    #[test]
    fn extract_strings_with_skip() {
        let args = vec![
            OscType::Int(0),
            OscType::String("a".into()),
            OscType::String("b".into()),
        ];
        assert_eq!(extract_strings(&args, 1), vec!["a", "b"]);
    }

    #[test]
    fn extract_floats_with_skip() {
        let args = vec![OscType::Int(0), OscType::Float(1.0), OscType::Float(2.0)];
        assert_eq!(extract_floats(&args, 1), vec![1.0, 2.0]);
    }

    #[test]
    fn extract_strings_zero_skip() {
        let args = vec![OscType::String("x".into()), OscType::String("y".into())];
        assert_eq!(extract_strings(&args, 0), vec!["x", "y"]);
    }
}
