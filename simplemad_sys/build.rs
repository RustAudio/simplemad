#[cfg(feature = "vendor")]
mod vendor {
    extern crate gcc;

    use std::env;

    pub fn vendor_libmad() {
        let mut build = gcc::Build::new();
        build.include("libmad-src");
        build.define("HAVE_CONFIG_H", None);

        let target = env::var("TARGET").unwrap();

        if target.contains("i686") {
            build
                .define("FPM_INTEL", None)
                .define("ASO_ZEROCHECK", None);
        } else if target.contains("x86_64") {
            build.define("FPM_64BIT", None);
        } else if target.contains("arm") {
            build.define("FPM_ARM", None);
        // TODO: I'm not sure if compiling assembly works
        //build.define("ASO_INTERLEAVE1", None);
        //build.define("ASO_IMDCT", None);
        //build.file("libmad-src/imdct_l_arm.S");
        } else if target.contains("mips") {
            build
                .define("FPM_MIPS", None)
                .define("ASO_INTERLEAVE2", None)
                .define("ASO_ZEROCHECK", None);
        } else if target.contains("sparc") {
            build.define("FPM_SPARC", None);
        } else if target.contains("ppc") || target.contains("powerpc") {
            build.define("FPM_PPC", None);
        } else {
            build.define("FPM_DEFAULT", None);
        };

        build
            .file("libmad-src/bit.c")
            .file("libmad-src/decoder.c")
            .file("libmad-src/fixed.c")
            .file("libmad-src/frame.c")
            .file("libmad-src/huffman.c")
            .file("libmad-src/layer12.c")
            .file("libmad-src/layer3.c")
            .file("libmad-src/stream.c")
            .file("libmad-src/synth.c")
            .file("libmad-src/timer.c")
            .file("libmad-src/version.c")
            .compile("libmad.a");
    }
}

#[cfg(not(feature = "vendor"))]
mod system {
    extern crate pkg_config;

    pub fn link_libmad() {
        pkg_config::find_library("mad").expect("Failed to link libmad, is it available?");
    }
}

fn main() {
    #[cfg(feature = "vendor")]
    vendor::vendor_libmad();
    #[cfg(not(feature = "vendor"))]
    system::link_libmad();
}
