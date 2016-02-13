extern crate lycan;
extern crate env_logger;
extern crate docopt;
extern crate rustc_serialize;

use std::io::{BufRead,Write};

use docopt::Docopt;

use lycan::game::{Game,GameParameters};

static USAGE: &'static str = r#"
Usage:
    lycan [options]

Options:
    -c URL, --configuration URL     URL of the configuration server [default: http://localhost:8000]
    -p PORT, --port PORT            Listening port [default: 7777]
    -h, --help                      Prints this message
"#;

#[derive(RustcDecodable,Debug)]
struct Args {
    flag_port: u16,
    flag_configuration: String,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "debug,mio=error");
    }
    env_logger::init().unwrap();
    let parameters = GameParameters {
        port: args.flag_port,
        configuration_url: args.flag_configuration.clone(),
    };
    let _request = Game::spawn_game(parameters);
    println!("Started game with parameters {:#?}", args);

    print!("Enter q to quit: ");
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    stdout.flush().unwrap();
    let lock = stdin.lock();
    for input in lock.lines() {
        match input.unwrap().as_ref() {
            "q" => break,
            _ => {
                print!("Enter q to quit: ");
                stdout.flush().unwrap();
                continue;
            }
        }
    }
}
