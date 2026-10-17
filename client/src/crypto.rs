use std::io::{self, Read, Write};

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};

const ID_SENDER: &[u8] = b"rbeam-sender-v1";
const ID_RECEIVER: &[u8] = b"rbeam-receiver-v1";
const CONFIRM: &[u8] = b"rbeam-secure-v1";
const MAX_HANDSHAKE_MSG: usize = 128;
const TAG_LEN: usize = 16;
const MAX_PLAINTEXT: usize = (1 << 20) - TAG_LEN;
const MAX_RECORD_LEN: usize = 1 << 20;

/// Authenticated, encrypted record stream over any byte channel.
pub struct SecureStream<S> {
    inner: S,
    read_cipher: ChaCha20Poly1305,
    write_cipher: ChaCha20Poly1305,
    read_nonce: u64,
    write_nonce: u64,
    read_buf: Vec<u8>,
    read_pos: usize,
    write_buf: Vec<u8>,
}

impl<S> SecureStream<S> {
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }
}

impl<S: Read + Write> SecureStream<S> {
    fn new(inner: S, read_key: &[u8], write_key: &[u8]) -> io::Result<Self> {
        let read_cipher = ChaCha20Poly1305::new_from_slice(read_key)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad read key"))?;
        let write_cipher = ChaCha20Poly1305::new_from_slice(write_key)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad write key"))?;

        Ok(Self {
            inner,
            read_cipher,
            write_cipher,
            read_nonce: 0,
            write_nonce: 0,
            read_buf: Vec::new(),
            read_pos: 0,
            write_buf: Vec::new(),
        })
    }

    fn nonce(counter: u64) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[4..].copy_from_slice(&counter.to_be_bytes());
        nonce
    }

    fn write_record(&mut self, plaintext: &[u8]) -> io::Result<()> {
        let nonce = Self::nonce(self.write_nonce);
        self.write_buf.clear();
        self.write_buf.extend_from_slice(plaintext);
        self.write_cipher
            .encrypt_in_place(Nonce::from_slice(&nonce), b"", &mut self.write_buf)
            .map_err(|_| io::Error::other("encryption failed"))?;

        let len = self.write_buf.len();
        if len > u32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "record too large",
            ));
        }

        self.inner.write_all(&(len as u32).to_be_bytes())?;
        self.inner.write_all(&self.write_buf)?;
        self.write_nonce = self
            .write_nonce
            .checked_add(1)
            .ok_or_else(|| io::Error::other("nonce exhausted"))?;
        Ok(())
    }

    fn read_record(&mut self) -> io::Result<()> {
        let mut len_buf = [0u8; 4];
        self.inner.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len <= TAG_LEN || len > MAX_RECORD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad record length",
            ));
        }

        self.read_buf.clear();
        self.read_buf.resize(len, 0);
        self.inner.read_exact(&mut self.read_buf)?;

        let nonce = Self::nonce(self.read_nonce);
        self.read_cipher
            .decrypt_in_place(Nonce::from_slice(&nonce), b"", &mut self.read_buf)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "decryption failed"))?;

        self.read_nonce = self
            .read_nonce
            .checked_add(1)
            .ok_or_else(|| io::Error::other("nonce exhausted"))?;
        self.read_pos = 0;
        Ok(())
    }
}

impl<S: Read + Write> Read for SecureStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.read_pos >= self.read_buf.len() {
            self.read_record()?;
        }

        let available = self.read_buf.len() - self.read_pos;
        let n = std::cmp::min(buf.len(), available);
        buf[..n].copy_from_slice(&self.read_buf[self.read_pos..self.read_pos + n]);
        self.read_pos += n;
        Ok(n)
    }
}

impl<S: Read + Write> Write for SecureStream<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut written = 0;
        while written < buf.len() {
            let end = std::cmp::min(written + MAX_PLAINTEXT, buf.len());
            self.write_record(&buf[written..end])?;
            written = end;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn derive_keys(shared: &[u8], initiator: bool) -> io::Result<([u8; 32], [u8; 32])> {
    let hk = Hkdf::<Sha256>::new(Some(b"rbeam-v1"), shared);
    let mut c2r = [0u8; 32];
    let mut r2c = [0u8; 32];

    hk.expand(b"rbeam c2r", &mut c2r)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "hkdf c2r failed"))?;
    hk.expand(b"rbeam r2c", &mut r2c)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "hkdf r2c failed"))?;

    if initiator {
        Ok((r2c, c2r))
    } else {
        Ok((c2r, r2c))
    }
}

fn write_frame<W: Write>(stream: &mut W, msg: &[u8]) -> io::Result<()> {
    if msg.len() > MAX_HANDSHAKE_MSG {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "handshake message too large",
        ));
    }
    stream.write_all(&(msg.len() as u16).to_be_bytes())?;
    stream.write_all(msg)?;
    stream.flush()
}

fn read_frame<R: Read>(stream: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf)?;
    let len = u16::from_be_bytes(len_buf) as usize;

    if len == 0 || len > MAX_HANDSHAKE_MSG {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad handshake length",
        ));
    }

    let mut msg = vec![0u8; len];
    stream.read_exact(&mut msg)?;
    Ok(msg)
}

fn confirm<S: Read + Write>(secure: &mut SecureStream<S>, initiator: bool) -> io::Result<()> {
    if initiator {
        secure.write_all(CONFIRM)?;
        secure.flush()?;
    }

    let mut peer = [0u8; CONFIRM.len()];
    secure.read_exact(&mut peer)?;
    if peer != CONFIRM {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "code mismatch",
        ));
    }

    if !initiator {
        secure.write_all(CONFIRM)?;
        secure.flush()?;
    }

    Ok(())
}

pub fn handshake_initiator<S: Read + Write>(
    mut stream: S,
    secret: &[u8],
) -> io::Result<SecureStream<S>> {
    let (state, msg_a) = Spake2::<Ed25519Group>::start_a(
        &Password::new(secret),
        &Identity::new(ID_SENDER),
        &Identity::new(ID_RECEIVER),
    );

    write_frame(&mut stream, &msg_a)?;
    let msg_b = read_frame(&mut stream)?;
    let shared = state
        .finish(&msg_b)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "spake2 failed"))?;

    let (read_key, write_key) = derive_keys(&shared, true)?;
    let mut secure = SecureStream::new(stream, &read_key, &write_key)?;
    confirm(&mut secure, true)?;
    Ok(secure)
}

pub fn handshake_responder<S: Read + Write>(
    mut stream: S,
    secret: &[u8],
) -> io::Result<SecureStream<S>> {
    let (state, msg_b) = Spake2::<Ed25519Group>::start_b(
        &Password::new(secret),
        &Identity::new(ID_SENDER),
        &Identity::new(ID_RECEIVER),
    );

    write_frame(&mut stream, &msg_b)?;
    let msg_a = read_frame(&mut stream)?;
    let shared = state
        .finish(&msg_a)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "spake2 failed"))?;

    let (read_key, write_key) = derive_keys(&shared, false)?;
    let mut secure = SecureStream::new(stream, &read_key, &write_key)?;
    confirm(&mut secure, false)?;
    Ok(secure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    fn pair(
        secret_a: &[u8],
        secret_b: &[u8],
    ) -> (SecureStream<TcpStream>, SecureStream<TcpStream>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let secret_a = secret_a.to_vec();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            handshake_initiator(stream, &secret_a)
        });

        let client = TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let responder = handshake_responder(client, secret_b).unwrap();
        let initiator = handle.join().unwrap().unwrap();

        (initiator, responder)
    }

    #[test]
    fn secure_stream_roundtrip() {
        let (mut initiator, mut responder) = pair(b"secret", b"secret");
        let message = b"hello secure world";

        initiator.write_all(message).unwrap();
        initiator.flush().unwrap();

        let mut buf = vec![0u8; message.len()];
        responder.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, message);
    }

    #[test]
    fn wrong_code_fails() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            handshake_initiator(stream, b"secret-a").is_err()
        });

        let client = TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let responder = handshake_responder(client, b"secret-b");

        assert!(responder.is_err());
        assert!(handle.join().unwrap());
    }
}
