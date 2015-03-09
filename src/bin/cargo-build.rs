#![feature(path)]

extern crate "rustc-serialize" as rustc_serialize;

extern crate cargo;
extern crate cargo_build;
extern crate docopt;

use std::sync::Arc;
use std::path::PathBuf;

use cargo::ops::{self, CompileOptions, ExecEngine};
use cargo::util::important_paths::{find_root_manifest_for_cwd};
use cargo::util::Config;

use docopt::Docopt;

use cargo_build::BuildEngine;

#[derive(RustcDecodable)]
struct Options {
    flag_package: Option<String>,
    flag_jobs: Option<u32>,
    flag_features: Vec<String>,
    flag_no_default_features: bool,
    flag_target: Option<String>,
    flag_manifest_path: Option<String>,
    flag_verbose: bool,
    flag_release: bool,
    flag_lib: bool,
    flag_emcc: Option<String>,
    flag_sysroot: Option<String>,
    flag_emit: Option<String>,
    flag_opt: Option<String>,
}

pub const USAGE: &'static str = "
Compile a local package and all of its dependencies

Usage:
    cargo-sysroot [options]

Options:
    -h, --help               Print this message
    -p SPEC, --package SPEC  Package to build
    -j N, --jobs N           The number of jobs to run in parallel
    --lib                    Build only lib (if present in package)
    --release                Build artifacts in release mode, with optimizations
    --features FEATURES      Space-separated list of features to also build
    --no-default-features    Do not build the `default` feature
    --target TRIPLE          Build for the target triple
    --manifest-path PATH     Path to the manifest to compile
    --sysroot SYSROOT        Sets the rustc sysroot path
    --emit TYPE              llvm35-ir, llvm-ir, llvm-bc, em-html, em-js, link (default)
    --opt OPT                Path to llvm `opt` executable
    --emcc EMCC              Use the `emcc` executable to compile an emscripten binary
    -v, --verbose            Use verbose output

If the --package argument is given, then SPEC is a package id specification
which indicates which package should be built. If it is not given, then the
current package is built. For more information on SPEC and its format, see the
`cargo help pkgid` command.
";

fn main() {
    let options: Options = Docopt::new(USAGE)
                                   .and_then(|d| d.decode())
                                   .unwrap_or_else(|e| e.exit());

    let mut shell = cargo::shell(options.flag_verbose);

    let root = find_root_manifest_for_cwd(options.flag_manifest_path).unwrap();

    let env = if options.flag_release || BuildEngine::emit_needs_35(&options.flag_emit) {
        "release"
    } else {
        "compile"
    };

    let engine = BuildEngine {
        sysroot: options.flag_sysroot.map(|s| PathBuf::new(&s)),
        emcc: options.flag_emcc.map(|s| PathBuf::new(&s)),
        opt: options.flag_opt.map(|s| PathBuf::new(&s)),
        emit: options.flag_emit,
    };

    let result = {
        let mut opts = CompileOptions {
            env: env,
            config: &Config::new(&mut shell).unwrap(),
            jobs: options.flag_jobs,
            target: options.flag_target.as_ref().map(|t| &t[..]),
            dev_deps: false,
            features: &options.flag_features[..],
            no_default_features: options.flag_no_default_features,
            spec: options.flag_package.as_ref().map(|s| &s[..]),
            lib_only: options.flag_lib,
            exec_engine: Some(Arc::new(Box::new(engine) as Box<ExecEngine>)),
        };

        ops::compile(&root, &mut opts)
    };
    
    cargo::process_executed(result.map(|_| None::<()>).map_err(|err| {
        cargo::util::CliError::from_boxed(err, 101)
    }), &mut shell);
}
