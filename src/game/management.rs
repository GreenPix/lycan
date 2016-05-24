use std::thread;
use std::sync::mpsc::{self,Sender};
use std::error::Error as StdError;

use mio::Sender as MioSender;
use serde_json::ser::to_vec_pretty;
use serde::Serialize;
use iron::prelude::*;
use iron::status::Status;
use iron::headers::ContentType;
use iron::{BeforeMiddleware};
use iron::error::HttpError;
use bodyparser::Struct;
use router::{Router};
use plugin::Extensible;
use modifier::Modifier;
use mount::Mount;

use lycan_serialize::AuthenticationToken;

use id::{Id,WeakId};
use messages::Request as LycanRequest;
use messages::Command;
use data::{ConnectCharacterParam,AuthenticatedRequest,Map};
use entity::Entity;
use instance::management::*;

// TODO
// - Set correct headers in all responses
// - Check if correct heahers are set (e.g. Content-Type)

pub fn start_management_api(sender: MioSender<LycanRequest>) {
    thread::spawn(move || {
        let router = create_router(sender);
        let mut mount = Mount::new();
        mount.mount("/api/v1", router);
        let mut chain = Chain::new(mount);
        chain.link_before(AuthenticationMiddleware("abcdefgh".to_string()));
        let mut error_router = ::iron_error_router::ErrorRouter::new();
        error_router.handle_status(Status::NotFound, |_: &mut Request| {
            Ok(Response::with((Status::NotFound, "404: Not Found")))
        });
        error_router.handle_status(Status::Unauthorized, |_: &mut Request| {
            Ok(Response::with((Status::Unauthorized, "401: Unauthorized")))
        });
        chain.link_after(error_router);

        let iron = Iron::new(chain);
        iron.http("127.0.0.1:8001").unwrap();
    });
}

macro_rules! itry_map {
    ($result:expr, |$err:ident| $bl:expr) => {
        match $result {
            ::std::result::Result::Ok(val) => val,
            ::std::result::Result::Err($err) => {
                return Ok(::iron::response::Response::with($bl));
            }
        }
    };
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
        rx.recv().unwrap()
    }};
    ($sender:ident, $id:ident, |$instance:ident| $bl:block) => {
        define_request_instance!($sender, $id, |$instance, _event_loop| $bl)
    };
}

// The Rust typechecker doesn't seem to get the types of the closures right
// It infers that they implement FnOnce(...), and therefore do not implement Handler
// This function forces the type of the closure
fn correct_bounds<F>(f: F) -> F
where F: Send + Sync + 'static + Fn(&mut Request) -> IronResult<Response>
{f}

fn create_router(sender: MioSender<LycanRequest>) -> Router {
    let mut server = Router::new();
    // TODO: Add middleware at the beginning for authentication of requests

    let clone = sender.clone();
    server.get("/maps", correct_bounds(move |_request| {
        let maps = define_request!(clone, |game| {
            game.resource_manager.get_all_maps()
        });
        Ok(Response::with((Status::Ok,JsonWriter(maps))))
    }));

    let clone = sender.clone();
    server.get("/maps/:id/instances", correct_bounds(move |request| {
        let params = request.extensions.get::<Router>().unwrap();
        // id is part of the route, the unwrap should never fail
        let id = &params["id"];
        let parsed = itry_map!(id.parse::<u64>(), |e| (Status::BadRequest, format!("ERROR: invalid id {}: {}", id, e)));
        let instances = define_request!(clone, |game| {
            game.get_instances(Id::forge(parsed))
        });
        Ok(Response::with((Status::Ok,JsonWriter(instances))))
    }));

    let clone = sender.clone();
    server.get("/instances/:id/entities", correct_bounds(move |request| {
        // id is part of the route, the unwrap should never fail
        let params = request.extensions.get::<Router>().unwrap();
        let id = &params["id"];
        let parsed = itry_map!(id.parse::<u64>(), |e| (Status::BadRequest, format!("ERROR: invalid id {}: {}", id, e)));
        let entities = itry_map!(define_request_instance!(clone, parsed, |instance| {
            instance.get_entities()
            }),
            |_e| (Status::BadRequest, format!("ERROR: Non existent instance id {}", parsed)));
        Ok(Response::with((Status::Ok,JsonWriter(entities))))
    }));

    let clone = sender.clone();
    server.get("/players", correct_bounds(move |_request| {
        let entities: Vec<_> = define_request!(clone, |game| {
            game.players.values().cloned().collect()
        });
        Ok(Response::with((Status::Ok, JsonWriter(entities))))
    }));

    let clone = sender.clone();
    server.post("/instances/:id/spawn", correct_bounds(move |request| {
        use data::SpawnMonster;
        let (id_parsed, parsed_monster);

        {
            let params = request.extensions.get::<Router>().unwrap();
            // id is part of the route, the unwrap should never fail
            let id = &params["id"];
            id_parsed = itry_map!(id.parse::<u64>(), |e|
                                  (Status::BadRequest, format!("ERROR: invalid id {}: {}", id, e)));
        }
        {
            let maybe_monster = itry_map!(request.get::<Struct<SpawnMonster>>(), |e|
                                          (Status::BadRequest, format!("ERROR: JSON decoding error: {}", e)));
            parsed_monster = iexpect!(maybe_monster, (Status::BadRequest, "ERROR: No JSON body provided"));
        }
        let monster = itry_map!(
            define_request_instance!(clone, id_parsed, |instance| {
                instance.spawn_monster(parsed_monster)
            }),
            |_e| (Status::BadRequest, format!("ERROR: Non existent instance id {}", id_parsed)));
        Ok(Response::with((Status::Ok,JsonWriter(monster))))
    }));

    let clone = sender.clone();
    server.post("/shutdown", correct_bounds(move |_request| {
        define_request!(clone, |g, el| {
            g.start_shutdown(el);
        });
        Ok(Response::with((Status::Ok, "OK")))
    }));

    let clone = sender.clone();
    server.post("/connect_character", correct_bounds(move |request| {
        let maybe_params = itry_map!(request.get::<Struct<ConnectCharacterParam>>(), |e|
                                     (Status::BadRequest, format!("ERROR: JSON decoding error: {}", e)));
        let decoded = iexpect!(maybe_params, (Status::BadRequest, "ERROR: No JSON body provided"));
        debug!("Received request to /connect_character: {:?}", decoded);
        define_request!(clone, |game| {
            let id = Id::forge(decoded.id);
            let token = AuthenticationToken(decoded.token);
            game.connect_character(id, token);
        });
        Ok(Response::with((Status::Ok, "OK")))
    }));

    let clone = sender.clone();
    fn entity_delete(sender: &MioSender<LycanRequest>, request: &mut Request) -> Result<(),String> {
        let params = request.extensions.get::<Router>().unwrap();
        // id is part of the route, the unwrap should never fail
        let instance_id = {
            let id = &params["instance_id"];
            try!(id.parse::<u64>().map_err(|e| format!("ERROR: invalid instance id {}: {}", id, e)))
        };
        let entity_id: WeakId<Entity> = {
            let id = &params["entity_id"];
            try!(id.parse::<u64>().map_err(|e| format!("ERROR: invalid entity id {}: {}", id, e))).into()
        };
        let result = try!(define_request_instance!(sender, instance_id, |instance| {
            instance.remove_entity(entity_id)
        }).map_err(|_e| format!("ERROR: Non existent instance id {}", instance_id)));
        result.map_err(|e| {
            match e {
                RemoveEntityError::NotFound => format!("ERROR: Entity {} not found in instance {}", entity_id, instance_id),
                RemoveEntityError::IsPlayer => format!("ERROR: Entity {} is a player", entity_id),
            }
        })
    }
    server.delete("/instances/:instance_id/entities/:entity_id", correct_bounds(move |request| {
        match entity_delete(&clone, request) {
            Ok(()) => Ok(Response::with((Status::Ok,"OK"))),
            Err(s) => Ok(Response::with((Status::BadRequest, s))),
        }
    }));

    server
}

struct JsonWriter<T>(T);

impl <T: Serialize> Modifier<Response> for JsonWriter<T> {
    fn modify(self, response: &mut Response) {
        match to_vec_pretty(&self.0) {
            Ok(v) => {
                response.headers.set(ContentType::json());
                v.modify(response);
            }
            Err(e) => {
                let err = format!("ERROR: JSON serialization error {}", e);
                let modifier = (Status::InternalServerError, err);
                modifier.modify(response);
            }
        }
    }
}

struct AuthenticationMiddleware(String);

impl BeforeMiddleware for AuthenticationMiddleware {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        match req.headers.get::<AccessToken>() {
            None => Err(IronError::new(AuthenticationError::NoToken, Status::Unauthorized)),
            Some(token) => {
                if &token.0 == &self.0 {
                    Ok(())
                } else {
                    Err(IronError::new(AuthenticationError::InvalidToken(token.0.clone()), Status::Unauthorized))
                }
            }
        }
    }
}

header! { (AccessToken, "Access-Token") => [String] }

#[derive(Debug,Clone)]
enum AuthenticationError {
    NoToken,
    InvalidToken(String),
}

impl StdError for AuthenticationError {
    fn description(&self) -> &str {
        use self::AuthenticationError::*;
        match *self {
            NoToken => "No authentication token",
            InvalidToken(_) => "Invalid authentication token",
        }
    }
}

impl ::std::fmt::Display for AuthenticationError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(),::std::fmt::Error> {
        use self::AuthenticationError::*;
        match *self {
            NoToken => write!(f, "No authentication token"),
            InvalidToken(ref t) => write!(f, "Invalid authentication token {}", t),
        }
    }
}
