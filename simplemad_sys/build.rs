extern crate gcc;
extern crate pkg_config;

use std::process::Command;
use std::path::Path;
use std::env;

fn main() {
    let out_dir_str = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    match pkg_config::find_library("mad") {
        Ok(_) => return,
        Err(..) => {}
    };

    if cfg!(windows) {
    panic!("Building libmad on Windows not implemented yet. Please install libmad manually.");
    }
    match env::set_current_dir("libmad-src") {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    match Command::new("mkdir").arg("temp").output() {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    match Command::new("tar").arg("-xzf")
                             .arg("libmad-0.15.1b.patched.tar.gz")
                             .arg("-C")
                             .arg("temp")
                             .output() {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    match env::set_current_dir("temp/libmad-0.15.1b/") {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    gcc::Config::new()
                .file("bit.c")
                .file("decoder.c")
                .file("fixed.c")
                .file("frame.c")
                .file("huffman.c")
                .file("layer3.c")
                .file("layer12.c")
                .file("stream.c")
                .file("synth.c")
                .file("timer.c")
                .file("version.c")
                .define("FPM_DEFAULT", None)
                .compile("libmad.a");

    match env::set_current_dir("../..") {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    match Command::new("rm").arg("-rf").arg("temp").output() {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    let out_str = out_dir.to_str().unwrap();
    println!("cargo:rustc-flags=-L native={}/lib -l static=mad", out_str);
    println!("cargo:rustdoc-flags=-L native={}/lib -l static=mad", out_str);
}
