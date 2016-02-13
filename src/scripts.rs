// Aariba scripts used by Lycan
use std::io::Read;

use aariba::rules::RulesEvaluator;
use aariba::parser;
use aariba::parser::ParseError;
use hyper::Client;
use hyper::error::Error as HyperError;

#[derive(Debug,Clone)]
pub struct AaribaScripts {
    pub combat: RulesEvaluator,
}

#[derive(Debug)]
pub enum Error {
    Hyper(HyperError),
    Parsing(ParseError),
}

impl From<HyperError> for Error {
    fn from(e: HyperError) -> Error {
        Error::Hyper(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Error {
        Error::Parsing(e)
    }
}

impl AaribaScripts {
    pub fn get_from_url(base_url: &str) -> Result<AaribaScripts,Error> {
        let mut url = String::from(base_url);
        url.push_str("/combat.aariba");
        debug!("Getting file {}", url);
        let script = try!(get_file_from_url(&url));
        let parsed_script = try!(parser::rules_evaluator(&script));
        let scripts = AaribaScripts {
            combat: parsed_script,
        };
        Ok(scripts)
    }
}

fn get_file_from_url(url: &str) -> Result<String,HyperError> {
    let client = Client::new();
    let mut response = try!(client.get(url).send());
    if !response.status.is_success() {
        return Err(HyperError::Status);
    }
    let mut content = String::new();
    try!(response.read_to_string(&mut content));
    Ok(content)
}
