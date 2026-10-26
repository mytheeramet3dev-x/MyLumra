use bytes::{Bytes, BytesMut};
use rustls::{ClientConfig, ClientConnection, ServerName};
use rustls::pki_types::ServerName as RustlsServerName;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct ManualTlsStream {
    tcp: TcpStream,
    tls: ClientConnection,
    read_buf: BytesMut,
    write_buf: BytesMut,
}

impl ManualTlsStream {
    pub async fn connect(host: &str, config: Arc<ClientConfig>) -> io::Result<Self> {
        let tcp = TcpStream::connect((host, 443)).await?;
        let server_name = RustlsServerName::try_from(host)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?
            .to_owned();

        let mut tls = ClientConnection::new(config, server_name)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut stream = ManualTlsStream {
            tcp,
            tls,
            read_buf: BytesMut::with_capacity(32 * 1024),
            write_buf: BytesMut::with_capacity(32 * 1024),
        };

        stream.handshake().await?;
        Ok(stream)
    }

    async fn handshake(&mut self) -> io::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        while self.tls.is_handshaking() {
            while self.tls.wants_write() {
                self.write_buf.clear();
                self.tls.write_tls(&mut self.write_buf)?;
                if !self.write_buf.is_empty() {
                    self.tcp.write_all(&self.write_buf).await?;
                }
            }

            if self.tls.wants_read() {
                let mut buf = [0u8; 8192];
                match self.tcp.read(&mut buf).await {
                    Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "tls eof")),
                    Ok(n) => {
                        self.tls.read_tls(&mut &buf[..n])?;
                        self.tls.process_new_packets()
                            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    pub async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut remaining = buf.len();
        let mut pos = 0;
        while remaining > 0 {
            match self.tls.reader().read(&mut buf[pos..]) {
                Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "tls closed")),
                Ok(n) => {
                    pos += n;
                    remaining -= n;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.fill_read_buf().await?;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn fill_read_buf(&mut self) -> io::Result<()> {
        if self.tls.wants_read() {
            let mut buf = [0u8; 8192];
            let n = self.tcp.read(&mut buf).await?;
            if n == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "tcp eof")); }
            self.tls.read_tls(&mut &buf[..n])?;
            self.tls.process_new_packets()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        Ok(())
    }

    pub async fn send_request(&mut self, req: &str) -> io::Result<()> {
        self.tls.writer().write_all(req.as_bytes())?;
        while self.tls.wants_write() {
            self.write_buf.clear();
            self.tls.write_tls(&mut self.write_buf)?;
            if !self.write_buf.is_empty() {
                self.tcp.write_all(&self.write_buf).await?;
            }
        }
        self.tcp.flush().await
    }

    pub async fn read_response(&mut self) -> io::Result<Bytes> {
        let mut response = Vec::new();
        let mut buf = [0u8; 16384];
        loop {
            match self.tls.reader().read(&mut buf) {
                Ok(n) => {
                    if n == 0 { break; }
                    response.extend_from_slice(&buf[..n]);
                    if response.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.fill_read_buf().await?;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Bytes::from(response))
    }
}

pub fn default_config() -> Arc<ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    )
}