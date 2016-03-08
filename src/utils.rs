use std::io::{Read,Write};
use std::fs::File;
use std::path::Path;

use hyper::Client;
use hyper::error::Error as HyperError;
use serde::{Serialize};
use serde_json;


pub fn get_file_from_url(url: &str) -> Result<String,HyperError> {
    let client = Client::new();
    let mut response = try!(client.get(url).send());
    if !response.status.is_success() {
        return Err(HyperError::Status);
    }
    let mut content = String::new();
    try!(response.read_to_string(&mut content));
    Ok(content)
}

pub fn serialize_to_file<T,P>(file: P, s: &T)
where T: Serialize,
      P: AsRef<Path> {
    if let Err(e) =  serialize_to_file_inner(file.as_ref(), s) {
        error!("Error when serializing: {}", e);
    }
}

fn serialize_to_file_inner<T>(file: &Path, s: &T) -> serde_json::Result<()>
where T: Serialize {
    let mut file = try!(File::create(file));
    serde_json::to_writer_pretty(&mut file, s)
}
