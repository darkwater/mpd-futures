use futures::Async::*;
use futures::{future, Poll, Future, IntoFuture, BoxFuture};
use std::io::{self, ErrorKind, Write};
use std::net::SocketAddr;
use std::str::FromStr;
use std::string::FromUtf8Error;
use tokio_core::io::{Io, Codec, Framed, EasyBuf};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_proto::pipeline::{ClientProto, ClientService};
use tokio_proto::TcpClient;
use tokio_service::Service;


#[derive(Default)]
struct MpdIdleCodec {
    did_read_version: bool,
    line_buffer: Vec<String>
}

#[derive(Debug)]
pub enum MpdError {
    Ack,
    Io(io::Error),
}

type MpdResponse = Vec<(String, String)>;

impl Codec for MpdIdleCodec {
    type In = Result<MpdResponse, MpdError>;
    type Out = String;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        // Ugly hack to read the initial version info line
        if !self.did_read_version {
            self.did_read_version = true;
            let _version = self.decode(buf);
        }

        // Attempt to read a line
        if let Some(length) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(length);
            let line = String::from_utf8(line.into()).unwrap();

            let _newline = buf.drain_to(1);

            if line.starts_with("OK") {
                let buf = self.line_buffer.split_off(0);

                let lines = buf.into_iter().map(|line| {
                    let mut split = line.splitn(2, ": ");
                    let key = split.next().unwrap().to_string();
                    let value = split.next().expect("expected value").to_string();

                    (key, value)
                }).collect();

                Ok(Some(Ok(lines)))
            } else if line.starts_with("ACK ") {
                Ok(Some(Err(MpdError::Ack)))
            } else {
                self.line_buffer.push(line);
                self.decode(buf)
            }
        } else {
            Ok(None)
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        writeln!(buf, "{}", msg);
        Ok(())
    }
}

#[derive(Default, Debug)]
struct MpdIdleProto;

impl<T: Io + 'static> ClientProto<T> for MpdIdleProto {
    type Request = String;
    type Response = Result<MpdResponse, MpdError>;
    type Transport = Framed<T, MpdIdleCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(MpdIdleCodec::default()))
    }
}

type MpdIdleService = ClientService<TcpStream, MpdIdleProto>;

#[derive(Clone, Debug)]
pub struct MpdIdle {
    serice: MpdIdleService
}

impl MpdIdle {
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> impl Future<Item=MpdIdle, Error=io::Error> {
        TcpStream::new(MpdIdleProto)
            .connect(addr, handle)
            .map(|service| {
                MpdIdle {
                    service: service
                }
            })
    }
}
