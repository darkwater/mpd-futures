#![feature(conservative_impl_trait)]
extern crate futures;
extern crate tokio_io;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

mod client;
mod convert;
// mod idle;
mod song;
mod status;

pub use client::*;
pub use convert::*;
// pub use idle::*;
pub use song::*;
pub use status::*;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use tokio_core::reactor::Core;

    #[test]
    fn it_works() {
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        let addr = "127.0.0.1:6600".parse().unwrap();
        let client = MpdClient::connect(&addr, &handle);

        let client = core.run(client).unwrap();
        println!("client: {:#?}", client);

        let play1 = client.play();
        let idle  = client.idle();
        let pause = client.pause();
        let play2 = client.play();

        let res = core.run(play1.join4(idle, pause, play2)).unwrap();

        println!("res: {:#?}", res);
    }
}
