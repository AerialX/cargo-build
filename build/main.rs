#![feature(path)]

extern crate git2;

use std::process::Command;
use std::fs::{ remove_dir_all, create_dir_all, metadata, rename };
use std::path::PathBuf;
use std::env;

use git2::Repository;

#[allow(deprecated)]
fn main() {
    let passes_name = "rust-emscripten-passes";
    let passes_url = "https://github.com/epdtry/rust-emscripten-passes.git";
    let passes = &[
        "RemoveAssume.so",
    ];

    let llvm_path = env::var("LLVM_PREFIX");
    let out_dir = PathBuf::new(&env::var_os("OUT_DIR").unwrap());
    let passes_dir = out_dir.join(passes_name);

    if let Ok(llvm_path) = llvm_path {
        println!("Cloning {}", passes_name);
        clone(passes_url, &passes_dir);

        println!("Compiling...");
        run(Command::new("make")
            .current_dir(&passes_dir)
            .arg(passes[0])
            .arg(passes[1])
            .arg(&format!("LLVM_PREFIX={}", llvm_path)));

        for pass in passes {
            rename(&passes_dir.join(pass), &out_dir.join("../../..").join(pass)).unwrap();
        }
    } else {
        println!("No LLVM_PREFIX specified, Emscripten and LLVM 3.5 output will fail");
    }
}

fn clone(url: &str, path: &PathBuf) -> Repository {
    if metadata(path).is_ok() {
        match Repository::open(path) {
            Ok(r) => {
                if !r.is_empty().unwrap() {
                    return r;
                }
            },
            _ => ()
        };

        remove_dir_all(path).unwrap();
    }

    create_dir_all(path).unwrap();
    Repository::clone(url, path).unwrap()
}

fn run(cmd: &mut Command) {
    println!("running: {:?}", cmd);
    assert!(cmd.status().unwrap().success());
}
