#![feature(conservative_impl_trait)]
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

mod client;

pub use client::*;

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
pub struct MpdCodec {
    did_read_version: bool,
    line_buffer: Vec<String>
}

#[derive(Debug)]
pub enum MpdError {
    Ack,
    Io(io::Error),
}

pub type MpdResponse = Vec<(String, String)>;

impl Codec for MpdCodec {
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
pub struct MpdClientProto;

impl<T: Io + 'static> ClientProto<T> for MpdClientProto {
    type Request = String;
    type Response = Result<MpdResponse, MpdError>;
    type Transport = Framed<T, MpdCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(MpdCodec::default()))
    }
}

type MpdService = ClientService<TcpStream, MpdClientProto>;

#[derive(Clone, Debug)]
pub struct MpdClient {
    service: MpdService
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Pause,
    Play,
    Stop,
}

impl FromStr for State {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use State::*;
        match s {
            "pause" => Ok(Pause),
            "play"  => Ok(Play),
            "stop"  => Ok(Stop),
            _       => Err(()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub volume:         Option<u64>,
    pub repeat:         Option<bool>,
    pub random:         Option<bool>,
    pub single:         Option<bool>,
    pub consume:        Option<bool>,
    pub playlist:       Option<usize>,
    pub playlistlength: Option<usize>,
    pub state:          Option<State>,
    pub playing:        Option<bool>, // Convenience fields
    pub paused:         Option<bool>, //
    pub stopped:        Option<bool>, //
    pub song:           Option<usize>,
    pub songid:         Option<usize>,
    pub nextsong:       Option<usize>,
    pub nextsongid:     Option<usize>,
    // time
    pub elapsed:        Option<f64>,
    pub duration:       Option<f64>,
    pub bitrate:        Option<u64>,
    pub xfade:          Option<u64>,
    // mixrampdb
    // mixrampdelay
    // audio
    // updating_db
    // error
}

#[derive(Clone, Debug, Default)]
pub struct Song {
    pub file:     Option<String>,
    pub artist:   Option<String>,
    pub album:    Option<String>,
    pub title:    Option<String>,
    pub disc:     Option<u64>,
    pub track:    Option<u64>,
    pub duration: Option<f64>,
    pub pos:      Option<usize>,
    pub id:       Option<usize>,
    pub tags:     Vec<(String, String)>,
}

trait IntoParameter {
    fn into_parameter(self) -> String;
}

impl IntoParameter for &'static str {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for f64 {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for u64 {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for bool {
    fn into_parameter(self) -> String {
        match self {
            true  => "1",
            false => "0",
        }.to_string()
    }
}

impl<A, B> IntoParameter for (A, B)
where A: IntoParameter,
      B: IntoParameter {
    fn into_parameter(self) -> String {
        format!("{} {}", self.0.into_parameter(), self.1.into_parameter())
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_core::reactor::Core;

    #[test]
    fn it_works() {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let addr = "127.0.0.1:6600".parse().unwrap();
        let client = MpdClient::connect(&addr, &handle)
            .then(|client| {
                let client = client.unwrap();

                client.setvol(80)
            });

        println!("{:#?}", core.run(client).unwrap());
    }
}
