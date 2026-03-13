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

/// Type-safe extraction from AbletonOSC reply args.
pub trait FromOsc: Sized {
    fn from_osc(args: &[OscType]) -> Result<Self, Error>;
}

impl FromOsc for f64 {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Float(f) => return Ok(f64::from(*f)),
                OscType::Double(d) => return Ok(*d),
                _ => continue,
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
        Err(Error::UnexpectedResponse(
            "expected int in OSC args".into(),
        ))
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
        Err(Error::UnexpectedResponse(
            "expected f32 in OSC args".into(),
        ))
    }
}

impl FromOsc for bool {
    fn from_osc(args: &[OscType]) -> Result<Self, Error> {
        for arg in args.iter().rev() {
            match arg {
                OscType::Bool(b) => return Ok(*b),
                OscType::Int(i) => return Ok(*i != 0),
                _ => continue,
            }
        }
        Err(Error::UnexpectedResponse(
            "expected bool in OSC args".into(),
        ))
    }
}

/// Extract all strings from args, skipping `skip` leading args (index prefix).
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

/// Extract all floats from args, skipping `skip` leading args.
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
        let send_addr: SocketAddr = SEND_ADDR.parse().expect("valid send address");
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(Self {
            socket,
            send_addr,
            pending: Arc::clone(&pending),
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
        {
            let mut map = self.pending.lock().await;
            map.insert(address.to_string(), tx);
        }

        self.send(address, args).await?;

        match tokio::time::timeout(QUERY_TIMEOUT, rx).await {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_)) | Err(_) => {
                let mut map = self.pending.lock().await;
                map.remove(address);
                Err(Error::OscTimeout)
            }
        }
    }

    /// Query and extract a typed value from the response using `FromOsc`.
    pub async fn query_val<T: FromOsc>(
        &self,
        address: &str,
        args: Vec<OscType>,
    ) -> Result<T, Error> {
        let msg = self.query(address, args).await?;
        T::from_osc(&msg.args)
    }
}
