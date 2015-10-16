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
        panic!("libmad could not be found. Building libmad automatically on Windows not implemented yet. Please install libmad manually.");
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

    Command::new("./configure")
        .args(&["--disable-shared", "--enable-static"]) // Only build static lib
        .args(&["--prefix", out_dir.to_str().unwrap()]) // Install on the outdir
        .arg("--with-pic") // Build position-independent code (required by Rust)
        .output()
        .unwrap();

    match Command::new("make").output() {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

    match Command::new("make").arg("install").output() {
        Ok(_) => {},
        Err(e) => panic!("{}", e)
    }

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
}
