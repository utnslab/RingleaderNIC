// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use bindgen::Builder;
use std::env;
use std::path::Path;
use std::process::Command;
use std::os;

extern crate gcc;

fn main() {
    let out_dir_s = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_s);

    let mut header_locations_str = env::var("RINGLEADER_DRIVER_DIR").unwrap() + "/src";
    let mut library_location_str = env::var("RINGLEADER_DRIVER_DIR").unwrap();
    let out_dir = Path::new(&out_dir_s);
    // it would be best to use full path
    let mut header_locations = vec![header_locations_str];

    let mut library_location = Some(library_location_str);
    let mut lib_names = vec!["dynamicixy"];

    // Step 1: Now that we've compiled ringleader user space driver , point cargo to the libraries.
    println!(
        "cargo:rustc-link-search=native={}",
        library_location.unwrap()
    );
    for lib_name in &lib_names {
        println!("cargo:rustc-link-lib=dylib={}", lib_name);
    }

    // Step 2: Generate bindings for the driver headers.
    let mut builder = Builder::default();
    for header_location in &header_locations {
        builder = builder.clang_arg(&format!("-I{}", header_location));
    }
    let bindings = builder
        .blocklist_type("rte_arp_ipv4")
        .blocklist_type("rte_arp_hdr")
        .clang_arg("-mavx")
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .unwrap_or_else(|e| panic!("Failed to generate bindings: {:?}", e));
    let bindings_out = out_dir.join("bindings.rs");
    bindings
        .write_to_file(bindings_out)
        .expect("Failed to write bindings");

    // Step 3: Compile a stub file so Rust can access `inline` functions in the headers
    // that aren't compiled into the libraries.
    let mut builder = cc::Build::new();
    builder.opt_level(3);
    builder.pic(true);
    builder.flag("-fomit-frame-pointer");
    builder.flag("-linker-plugin-lto");
    builder.flag("-march=native");
    builder.file("inlined.c");
    for header_location in &header_locations {
        builder.include(header_location);
    }
    builder.compile("inlined");
}
