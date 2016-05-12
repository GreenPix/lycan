use std::thread;
use std::sync::mpsc::{self,Sender};

use mio::Sender as MioSender;
use nickel::{Nickel,HttpRouter};
use serde_json::ser::to_string_pretty;
use nickel::JsonBody;

use lycan_serialize::AuthenticationToken;

use id::Id;
use messages::Request as LycanRequest;
use data::Map;

#[derive(Debug,RustcDecodable)]
struct AuthenticatedRequest<T> {
    secret: String,
    params: T,
}

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
    // TODO: Add middleware at the beginning for authentication of requests

    let clone = sender.clone();
    server.get("/maps", middleware! {
        let maps = define_request!(clone, |game| {
            game.resource_manager.get_all_maps()
        });
        let json = to_string_pretty(&maps).unwrap();
        json
    });

    let clone = sender.clone();
    server.get("/maps/:id/instances", middleware! { |request|
        // id is part of the route, the unwrap should never fail
        let id = request.param("id").unwrap();
        match id.parse::<u64>() {
            Ok(parsed) => {
                //let id_instance: Id<Instance> = Id::forge(parsed);
                let instances = define_request!(clone, |game| {
                    game.get_instances(Id::forge(parsed))
                });
                let json = to_string_pretty(&instances).unwrap();
                json
            }
            Err(e) => {
                format!("ERROR: invalid id {}", e)  // TODO: Do things properly (set error code ...)
            }
        }
    });

    let clone = sender.clone();
    server.post("/shutdown", middleware! {
        define_request!(clone, |g, el| {
            g.start_shutdown(el);
        });
        "OK"
    });

    let clone = sender.clone();
    #[derive(Debug,RustcDecodable)]
    struct ConnectCharacterParam {
        token: String,
        id: u64,
    }
    server.post("/connect_character", middleware! { |request|
        match request.json_as::<AuthenticatedRequest<ConnectCharacterParam>>() {
            Ok(decoded) => {
                debug!("Received request to /connect_character: {:?}", decoded);
                define_request!(clone, |game| {
                    let id = Id::forge(decoded.params.id);
                    let token = AuthenticationToken(decoded.params.token);
                    game.connect_character(id, token);
                });
                Ok("OK")
            }
            Err(e) => {
                debug!("Error while parsing body for /connect_character: {}", e);
                Err(e.to_string())
            }
        }
    });
}
