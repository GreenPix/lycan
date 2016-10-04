extern crate serde_codegen;

use std::env;
use std::path::Path;
use std::fs::DirBuilder;

const FILES: &'static [&'static str] = &[
    "src/data/management.rs.in",
    "src/data/player.rs.in",
    "src/data/map.rs.in",
    "src/data/monster.rs.in",
];
pub fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    for input_file in FILES {
        let src = Path::new(input_file);
        let dst = Path::new(&out_dir).join(input_file.trim_right_matches(".in"));
        let dst_dir = dst.parent().unwrap();
        DirBuilder::new().recursive(true).create(dst_dir).unwrap();

        serde_codegen::expand(&src, &dst).unwrap();
        println!("cargo:rerun-if-changed={}", input_file);
    }
}
