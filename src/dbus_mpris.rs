extern crate dbus;
extern crate dbus_tokio;
extern crate futures;
extern crate tokio_core;

use std::rc::Rc;
use dbus::{BusType, Connection, NameFlag};
use dbus_tokio::tree::{AFactory, ATree, ATreeServer};
use dbus_tokio::AConnection;
use tokio_core::reactor::Handle;
use librespot::connect::spirc::Spirc;

use futures::{Future, Stream};

pub fn create_server(handle: Handle, spirc: Rc<Spirc>) -> Box<Future<Item = (), Error = ()>> {
    let c = Rc::new(Connection::get_private(BusType::Session).unwrap());

    c.register_name(
        "org.mpris.MediaPlayer2.spotifyd",
        NameFlag::ReplaceExisting as u32,
    ).unwrap();

    let pause_spirc = spirc.clone();
    let prev_spirc = spirc.clone();
    let next_spirc = spirc.clone();
    let play_pause_spirc = spirc.clone();
    let play_spirc = spirc.clone();
    let f = AFactory::new_afn::<()>();
    let tree = f.tree(ATree::new()).add(
        f.object_path("/org/mpris/MediaPlayer2", ())
            .introspectable()
            .add(
                f.interface("org.mpris.MediaPlayer2.Player", ())
                    .add_m(f.amethod("Next", (), move |m| {
                        next_spirc.as_ref().next();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod("Previous", (), move |m| {
                        prev_spirc.as_ref().prev();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod("Pause", (), move |m| {
                        pause_spirc.as_ref().pause();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod("PlayPause", (), move |m| {
                        play_pause_spirc.as_ref().play_pause();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    }))
                    .add_m(f.amethod("Play", (), move |m| {
                        play_spirc.as_ref().play();
                        let mret = m.msg.method_return();
                        Ok(vec![mret])
                    })),
            ),
    );

    // We register all object paths in the tree.
    tree.set_registered(&c, true).unwrap();

    // Setup Tokio
    let aconn = AConnection::new(c.clone(), handle).unwrap();
    let server = ATreeServer::new(c.clone(), Box::new(tree), aconn.messages().unwrap());

    // Make the server run forever
    Box::new(server.for_each(|m| {
        warn!("Unhandled dbus message: {:?}", m);
        Ok(())
    }))
}
