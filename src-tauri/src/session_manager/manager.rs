use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::{client, kex, Preferred};
use russh::client::Config;
use russh::kex::{CURVE25519, DH_G14_SHA1, DH_G14_SHA256, DH_G1_SHA1};
use russh_keys::key::{ED25519, RSA_SHA2_256, RSA_SHA2_512, SignatureHash, SSH_RSA};
use uuid::Uuid;

use crate::device_manager::Device;
use crate::session_manager::{Error, ErrorKind, Proc, SessionManager, Shell, ShellToken};
use crate::session_manager::connection::Connection;
use crate::session_manager::handler::ClientHandler;

impl SessionManager {
    pub async fn exec(
        &self,
        device: Device,
        command: &str,
        stdin: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, Error> {
        loop {
            let conn = self.conn_obtain(device.clone()).await?;
            log::info!("obtained connection for command {}", command);
            match conn.exec(command, &stdin).await {
                Ok(data) => return Ok(data),
                Err(e) => match e.kind {
                    ErrorKind::NeedsReconnect => {
                        log::info!("retry connection");
                        continue;
                    }
                    _ => return Err(e)
                }
            };
        }
    }

    pub async fn proc_open(&self, device: Device, command: &str) -> Result<Proc, Error> {
        loop {
            let conn = self.conn_obtain(device.clone()).await?;
            log::info!("obtained connection for proc_open {}", command);
            match conn.open(command).await {
                Ok(data) => return Ok(data),
                Err(e) => match e.kind {
                    ErrorKind::NeedsReconnect => {
                        log::info!("retry connection");
                        continue;
                    }
                    _ => return Err(e)
                }
            };
        }
    }

    pub async fn shell_open(
        &self,
        device: &Device,
        cols: u16,
        rows: u16,
    ) -> Result<ShellToken, Error> {
        return Err(Error::unimplemented());
    }

    pub async fn shell_close(&self, token: &ShellToken) -> Result<(), Error> {
        return Err(Error::unimplemented());
    }

    pub fn shell_find(&self, token: &ShellToken) -> Option<Arc<Shell>> {
        return self.shells.read().unwrap().get(token).map(|a| a.clone());
    }

    pub fn shell_list(&self) -> Vec<ShellToken> {
        return self
            .shells
            .read()
            .unwrap()
            .keys()
            .map(|k| k.clone())
            .collect();
    }

    async fn conn_obtain(&self, device: Device) -> Result<Arc<Connection>, Error> {
        if let Some(a) = self.connections.lock().unwrap().get(&device.name) {
            return Ok(a.clone());
        }
        let connection = Arc::new(self.conn_new(device.clone()).await?);
        if !device.new {
            self.connections
                .lock()
                .unwrap()
                .insert(device.name, connection.clone());
        }
        return Ok(connection);
    }

    async fn conn_new(&self, device: Device) -> Result<Connection, Error> {
        let mut config = Config::default();
        config.preferred.key = &[ED25519, RSA_SHA2_512, RSA_SHA2_256, SSH_RSA];
        config.preferred.kex = &[DH_G14_SHA1, DH_G1_SHA1, CURVE25519, DH_G14_SHA256];
        config.connection_timeout = Some(Duration::from_secs(3));
        let last_server_hash_alg: Arc<Mutex<Option<SignatureHash>>> = Arc::new(Mutex::default());
        let handler = ClientHandler {
            id: device.name.clone(),
            connections: Arc::downgrade(&self.connections),
            hash_alg: last_server_hash_alg.clone(),
        };
        let addr = SocketAddr::from_str(&format!("{}:{}", &device.host, &device.port)).unwrap();
        log::debug!("Connecting to {}", addr);
        let mut handle = client::connect(Arc::new(config), addr, handler).await?;
        log::debug!("Connected to {}, sig_alg: {:?}", addr, last_server_hash_alg.lock().unwrap());
        if let Some(key) = &device.private_key {
            let key = Arc::new(key.priv_key(device.passphrase.as_deref(),
                                            last_server_hash_alg.lock().unwrap().clone())?);
            log::debug!("Key algorithm: {:?}", key.name());
            if !handle.authenticate_publickey(&device.username, key).await? {
                return Err(Error {
                    message: format!("Device refused pubkey authorization"),
                    kind: ErrorKind::Authorization,
                });
            }
        } else if let Some(password) = &device.password {
            if !handle.authenticate_password(&device.username, password).await? {
                return Err(Error {
                    message: format!("Device refused password authorization"),
                    kind: ErrorKind::Authorization,
                });
            }
        } else if !handle.authenticate_none(&device.username).await? {
            return Err(Error {
                message: format!("Device refused authorization"),
                kind: ErrorKind::Authorization,
            });
        }
        log::debug!("Authenticated to {}", addr);
        return Ok(Connection::new(device, handle, Arc::downgrade(&self.connections)));
    }
}
