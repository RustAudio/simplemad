extern crate gcc;
extern crate pkg_config;

use std::env;

fn main() {
    match pkg_config::find_library("mad") {
        Ok(_) => return,
        Err(..) => {}
    };

    // attempt to compile libmad manually
    let mut gcc = gcc::Config::new();
    gcc.include("libmad-src");

    let target = env::var("TARGET").unwrap();

    if target.contains("i686") {
        gcc.define("FPM_INTEL", None);
        gcc.define("ASO_ZEROCHECK", None);

    } else if target.contains("x86_64") {
        gcc.define("FPM_64BIT", None);

    } else if target.contains("arm") {
        gcc.define("FPM_ARM", None);
        // TODO: I'm not sure if compiling assembly works
        //gcc.define("ASO_INTERLEAVE1", None);
        //gcc.define("ASO_IMDCT", None);
        //gcc.file("libmad-src/imdct_l_arm.S");

    } else if target.contains("mips") {
        gcc.define("FPM_MIPS", None);
        gcc.define("ASO_INTERLEAVE2", None);
        gcc.define("ASO_ZEROCHECK", None);

    } else if target.contains("sparc") {
        gcc.define("FPM_SPARC", None);

    } else if target.contains("ppc") || target.contains("powerpc") {
        gcc.define("FPM_PPC", None);

    } else {
        gcc.define("FPM_DEFAULT", None);
    };

    gcc.file("libmad-src/version.c")
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
