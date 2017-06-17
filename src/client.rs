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
use tokio_proto::multiplex::{ClientProto, ClientService};
use tokio_proto::TcpClient;
use tokio_service::Service;

use song::*;
use status::*;
use convert::*;


#[derive(Default)]
struct MpdClientCodec {
    did_read_version: bool,
    line_buffer: Vec<String>,
    last_index: u64,
    idling: bool,
    idle_break: bool,
    idle_index: u64,
}

#[derive(Debug)]
pub enum MpdError {
    Ack,
    Io(io::Error),
}

type MpdResponse = Vec<(String, String)>;

impl Codec for MpdClientCodec {
    type In = (u64, Result<MpdResponse, MpdError>);
    type Out = (u64, String);

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        println!("{} bytes available: {:?}", buf.len(), String::from_utf8(buf.as_slice().to_vec()).unwrap());
        // Ugly hack to read the initial version info line
        if !self.did_read_version {
            self.did_read_version = true;

            if let Some(length) = buf.as_slice().iter().position(|&b| b == b'\n') {
                let line = buf.drain_to(length);
                let line = String::from_utf8(line.into()).unwrap();
                let _newline = buf.drain_to(1);

                println!("version: {}", line);

                return Ok(None);
            } else {
                return Ok(None);
            }
        }

        let ret_val;

        // Attempt to read a line
        if let Some(length) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(length);
            let line = String::from_utf8(line.into()).unwrap();

            let _newline = buf.drain_to(1);
            let index = if self.idling == self.idle_break {
                self.last_index
            } else {
                self.idle_index
            };

            if line.starts_with("OK") {
                let buf = self.line_buffer.split_off(0);

                let lines = buf.into_iter().map(|line| {
                    let mut split = line.splitn(2, ": ");
                    let key = split.next().unwrap().to_string();
                    let value = split.next().expect("expected value").to_string();

                    (key, value)
                }).collect();

                println!("{} => {:?}", index, lines);
                ret_val = Ok(Some((index, Ok(lines))));
                self.last_index += 1;
            } else if line.starts_with("ACK ") {
                ret_val = Ok(Some((index, Err(MpdError::Ack))));
                self.last_index += 1;
            } else {
                self.line_buffer.push(line);
                ret_val = self.decode(buf);
            }
        } else {
            ret_val = Ok(None);
        }

        ret_val
    }

    fn encode(&mut self, (index, msg): Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        println!("{} => {}", index, msg);

        if msg == "idle" {
            self.idling = true;
            self.idle_index = index;
        }

        if self.idling { writeln!(buf, "noidle"); }

        writeln!(buf, "{}", msg);

        if self.idling { writeln!(buf, "idle"); }

        Ok(())
    }
}

#[derive(Default, Debug)]
struct MpdClientProto;

impl<T: Io + 'static> ClientProto<T> for MpdClientProto {
    type Request = String;
    type Response = Result<MpdResponse, MpdError>;
    type Transport = Framed<T, MpdClientCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(MpdClientCodec::default()))
    }
}

type MpdClientService = ClientService<TcpStream, MpdClientProto>;

#[derive(Clone, Debug)]
pub struct MpdClient {
    service: MpdClientService
}

impl MpdClient {
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> impl Future<Item=MpdClient, Error=io::Error> {
        TcpClient::new(MpdClientProto)
            .connect(addr, handle)
            .map(|service| {
                MpdClient {
                    service: service
                }
            })
    }

    fn send_command<P>(&self, cmd: P) -> impl Future<Item=MpdResponse, Error=MpdError>
    where P: IntoParameter {
        self.service.call(cmd.into_parameter())
            .then(|result| {
                match result {
                    Ok(Ok(res)) => future::ok(res),
                    Ok(Err(e))  => future::err(e),
                    Err(e)      => future::err(MpdError::Io(e)),
                }
            })
    }

    pub fn play(&self) -> impl Future<Item=(), Error=MpdError> {
        self.send_command("play").map(|_| ())
    }

    pub fn pause(&self) -> impl Future<Item=(), Error=MpdError> {
        self.send_command("pause").map(|_| ())
    }

    pub fn next(&self) -> impl Future<Item=(), Error=MpdError> {
        self.send_command("next").map(|_| ())
    }

    pub fn prev(&self) -> impl Future<Item=(), Error=MpdError> {
        self.send_command("previous").map(|_| ())
    }

    pub fn seekcur(&self, seek: f64) -> impl Future<Item=(), Error=MpdError> {
        self.send_command(("seekcur", seek)).map(|_| ())
    }

    pub fn setvol(&self, volume: u64) -> impl Future<Item=(), Error=MpdError> {
        self.send_command(("setvol", volume)).map(|_| ())
    }

    pub fn repeat(&self, repeat: bool) -> impl Future<Item=(), Error=MpdError> {
        self.send_command(("repeat", repeat)).map(|_| ())
    }

    pub fn random(&self, random: bool) -> impl Future<Item=(), Error=MpdError> {
        self.send_command(("random", random)).map(|_| ())
    }

    pub fn status(&self) -> impl Future<Item=Status, Error=MpdError> {
        self.send_command("status").map(|result| {
                let mut status = Status::default();

                for (key, value) in result.into_iter() {
                    match &*key {
                        "volume"         => status.volume         = Some(value.parse().unwrap()),
                        "repeat"         => status.repeat         = Some(value.parse::<u8>().unwrap() == 1),
                        "random"         => status.random         = Some(value.parse::<u8>().unwrap() == 1),
                        "single"         => status.single         = Some(value.parse::<u8>().unwrap() == 1),
                        "consume"        => status.consume        = Some(value.parse::<u8>().unwrap() == 1),
                        "playlist"       => status.playlist       = Some(value.parse().unwrap()),
                        "playlistlength" => status.playlistlength = Some(value.parse().unwrap()),
                        "state"          => status.state          = Some(value.parse().unwrap()),
                        "song"           => status.song           = Some(value.parse().unwrap()),
                        "songid"         => status.songid         = Some(value.parse().unwrap()),
                        "nextsong"       => status.nextsong       = Some(value.parse().unwrap()),
                        "nextsongid"     => status.nextsongid     = Some(value.parse().unwrap()),
                        "elapsed"        => status.elapsed        = Some(value.parse().unwrap()),
                        "duration"       => status.duration       = Some(value.parse().unwrap()),
                        "bitrate"        => status.bitrate        = Some(value.parse().unwrap()),
                        "xfade"          => status.xfade          = Some(value.parse().unwrap()),
                        _                => ()
                    }
                }

                if let Some(state) = status.state.as_ref() {
                    status.playing = Some(state == &State::Play);
                    status.paused  = Some(state == &State::Pause);
                    status.stopped = Some(state == &State::Stop);
                }

                status
            })
    }

    pub fn currentsong(&self) -> impl Future<Item=Option<Song>, Error=MpdError> {
        self.send_command("currentsong").map(|result| {
                if result.len() == 0 { return None }

                let mut song = Song::default();

                for (key, value) in result.into_iter() {
                    match &*key {
                        "file"     => song.file     = Some(value.parse().unwrap()),
                        "Artist"   => song.artist   = Some(value.parse().unwrap()),
                        "Album"    => song.album    = Some(value.parse().unwrap()),
                        "Title"    => song.title    = Some(value.parse().unwrap()),
                        "Disc"     => song.disc     = Some(value.parse().unwrap()),
                        "Track"    => song.track    = Some(value.parse().unwrap()),
                        "duration" => song.duration = Some(value.parse().unwrap()),
                        "Pos"      => song.pos      = Some(value.parse().unwrap()),
                        "Id"       => song.id       = Some(value.parse().unwrap()),
                        _          => song.tags.push((key, value))
                    }
                }

                Some(song)
            })
    }

    pub fn idle(&self) -> impl Future<Item=MpdResponse, Error=MpdError> {
        self.send_command("idle")
    }
}
