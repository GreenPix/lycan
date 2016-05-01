use std::thread;
use std::sync::mpsc::{self,Sender};

use mio::Sender as MioSender;
use nickel::{Nickel,HttpRouter};
use serde_json::ser::to_string_pretty;

use messages::Request as LycanRequest;
use data::Map;

pub fn start_management_api(sender: MioSender<LycanRequest>) {
    thread::spawn(move || {
        let mut server = Nickel::new();
        add_management_routes(&mut server, sender);

        server.listen("127.0.0.1:8001");
    });
}

/// Macro to reduce the boilerplate of creating a channel, create a request, send it to Game and
/// wait for the response
macro_rules! define_request {
    ($sender:ident, |$game:ident, $event_loop:ident| $bl:block) => {{
        let (tx, rx) = mpsc::channel();
        let request = LycanRequest::new(move |$game, $event_loop| {
            let result = $bl;
            let _ = tx.send(result);
        });
        $sender.send(request).unwrap();
        rx.recv().unwrap()
    }};
    ($sender:ident, |$game:ident| $bl:block) => {
        define_request!($sender, |$game, _event_loop| $bl)
    };
}

fn add_management_routes(server: &mut Nickel, sender: MioSender<LycanRequest>) {
    let clone = sender.clone();
    server.get("/maps", middleware! {
        let maps = define_request!(clone, |game| {
            game.resource_manager.get_all_maps()
        });
        let json = to_string_pretty(&maps).unwrap();
        json
    });

    let clone = sender.clone();
    server.post("/shutdown", middleware! {
        define_request!(clone, |g, el| {
            g.start_shutdown(el);
        });
        "OK"
    });
}
