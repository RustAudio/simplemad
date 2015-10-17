extern crate gcc;
extern crate pkg_config;

use std::env;

fn main() {
    match pkg_config::find_library("mad") {
        Ok(_) => return,
        Err(..) => {}
    };

    let target = env::var("TARGET").unwrap();
    let fpm = if target.contains("i686") {
        "FPM_INTEL"
    } else if target.contains("x86_64") {
        "FPM_64BIT"
    } else if target.contains("arm") {
        "FPM_ARM"
    } else if target.contains("mips") {
        "FPM_MIPS"
    } else if target.contains("sparc") {
        "FPM_SPARC"
    } else if target.contains("ppc") || target.contains("powerpc") {
        "FPM_PPC"
    } else {
        "FPM_DEFAULT"
    };

    gcc::Config::new()
                .include("libmad-src")
                .define(&fpm, None)
                .file("libmad-src/version.c")
                .file("libmad-src/fixed.c")
                .file("libmad-src/bit.c")
                .file("libmad-src/timer.c")
                .file("libmad-src/stream.c")
                .file("libmad-src/frame.c")
                .file("libmad-src/synth.c")
                .file("libmad-src/decoder.c")
                .file("libmad-src/layer12.c")
                .file("libmad-src/layer3.c")
                .file("libmad-src/huffman.c")
                .compile("libmad.a");  // the official name is `libmad.la` but gcc-rs forbids that
}
