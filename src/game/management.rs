use std::thread;
use std::sync::mpsc::{self,Sender};

use mio::Sender as MioSender;
use nickel::{Nickel,HttpRouter};
use serde_json::ser::to_string_pretty;
use nickel::JsonBody;

use lycan_serialize::AuthenticationToken;

use id::Id;
use messages::Request as LycanRequest;
use messages::Command;
use data::Map;

// TODO
// - Set correct headers in all responses
// - Check if correct heahers are set (e.g. Content-Type)
// - Authentication of each request
// - Do proper error handling

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

/// Macro to reduce the boilerplate of creating a channel, create a request, send it to Game
/// Route it to the correct Instance and wait for the response
macro_rules! define_request_instance {
    ($sender:ident, $id:ident, |$instance:ident, $event_loop:ident| $bl:block) => {{
        let (tx, rx) = mpsc::channel();
        let request = LycanRequest::new(move |g, _el| {
            let instance = match g.instances.get(&$id) {
                Some(i) => i,
                None => { let _ = tx.send(Err(())); return; }
            };
            let command = Command::new(move |$instance, $event_loop| {
                let result = $bl;
                let _ = tx.send(Ok(result));
            });
            let _ = instance.send(command);
        });
        $sender.send(request).unwrap();
        rx.recv().unwrap().unwrap()
    }};
    ($sender:ident, $id:ident, |$instance:ident| $bl:block) => {
        define_request_instance!($sender, $id, |$instance, _event_loop| $bl)
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
    server.get("/instances/:id/entities", middleware! { |request|
        // id is part of the route, the unwrap should never fail
        let id = request.param("id").unwrap();
        match id.parse::<u64>() {
            Ok(parsed) => {
                let entities = define_request_instance!(clone, parsed, |instance| {
                    instance.get_entities()
                });
                let json = to_string_pretty(&entities).unwrap();
                json
            }
            Err(e) => {
                format!("ERROR: invalid id {}", e)  // TODO: Do things properly (set error code ...)
            }
        }
    });

    let clone = sender.clone();
    fn monster_spawn(sender: &MioSender<LycanRequest>, request: &mut ::nickel::Request) -> Result<String,String> {
        use data::SpawnMonster;
        use serde_json;
        // id is part of the route, the unwrap should never fail
        let monster: SpawnMonster = try!(serde_json::from_reader(&mut request.origin).map_err(|e| format!("ERROR: bad input {}", e)));
        let id = request.param("id").unwrap();
        let id_parsed = try!(id.parse::<u64>().map_err(|e| format!("ERROR: invalid id {}", e)));
        let monster = define_request_instance!(sender, id_parsed, |instance| {
            instance.spawn_monster(monster)
        });
        let json = to_string_pretty(&monster).unwrap();
        Ok(json)
    }
    server.post("/instances/:id/spawn", middleware! { |request|
        match monster_spawn(&clone, request) {
            Ok(s) => s,
            Err(s) => s,
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

    let clone = sender.clone();
    fn entity_delete(sender: &MioSender<LycanRequest>, request: &mut ::nickel::Request) -> Result<(),String> {
        let instance_id = {
            let id = request.param("instance_id").unwrap();
            try!(id.parse::<u64>().map_err(|e| format!("ERROR: invalid instance id {}", e)))
        };
        let entity_id = {
            let id = request.param("entity_id").unwrap();
            try!(id.parse::<u64>().map_err(|e| format!("ERROR: invalid entity id {}", e)))
        };
        let result = define_request_instance!(sender, instance_id, |instance| {
            instance.remove_entity(entity_id)
        });
        result.map_err(|_| format!("ERROR: Entity {} not found in instance {}", entity_id, instance_id))
    }
    server.delete("/instances/:instance_id/entities/:entity_id", middleware! { |request|
        match entity_delete(&clone, request) {
            Ok(()) => "OK".to_string(),
            Err(s) => s,
        }
    });
}
