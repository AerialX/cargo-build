#![feature(os)]

extern crate cargo;

use std::process::{Output, Command, Stdio};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{self, BufReader, BufRead, BufWriter, Write, copy};
use std::os::self_exe_path;
use std::mem::swap;

use cargo::ops::{ExecEngine, CommandPrototype, CommandType};
use cargo::util::{self, ProcessError, ProcessBuilder};

pub struct BuildEngine {
    pub target: Option<String>,
    pub sysroot: Option<PathBuf>,
    pub emcc: Option<PathBuf>,
    pub opt: Option<PathBuf>,
    pub emit: Option<String>,
}

impl ExecEngine for BuildEngine {
    fn exec(&self, command: CommandPrototype) -> Result<(), ProcessError> {
        exec(command, false, self).map(|_| ())
    }

    fn exec_with_output(&self, command: CommandPrototype) -> Result<Output, ProcessError> {
        exec(command, true, self).map(|a| a.unwrap())
    }
}

impl BuildEngine {
    pub fn emit_needs_35(emit: &Option<String>) -> bool {
        match *emit {
            Some(ref emit) if emit.starts_with("llvm35-") || emit.starts_with("em-") => true,
            _ => false,
        }
    }
}

fn exec(mut command: CommandPrototype, with_output: bool, engine: &BuildEngine) -> Result<Option<Output>, ProcessError> {
    match command.get_type() {
        &CommandType::Rustc => (),
        _ => return do_exec(command.into_process_builder(), with_output),
    }

    let is_binary = command.get_args().windows(2)
        .find(|&args| {
            args[0].to_str() == Some("--crate-type") &&
                args[1].to_str() == Some("bin")
        }).is_some();

    // finding crate name
    let crate_name = command.get_args().windows(2)
        .filter_map(|args| {
            if args[0].to_str() == Some("--crate-name") {
                Some(args[1].to_str().unwrap().to_string())
            } else {
                None
            }
        }).next().unwrap();

    // finding out dir
    let out_dir = command.get_args().windows(2)
        .filter_map(|args| {
            if args[0].to_str() == Some("--out-dir") {
                Some(args[1].to_str().unwrap().to_string())
            } else {
                None
            }
        }).next().unwrap();

    let has_target = command.get_args()
        .iter().find(|&arg| {
            arg.to_str() == Some("--target")
        }).is_some();

    // NOTE: this is a hack, I'm not sure if there's a better way to detect this...
    // We don't want to inject --sysroot into build dependencies meant to run on the target machine
    let is_build = crate_name == "build-script-build" || (!has_target && engine.target.is_some());

    let (emit, rustc_emit, transform) = {
        if is_binary && !is_build {
            if BuildEngine::emit_needs_35(&engine.emit) {
                (engine.emit.as_ref(), Some("llvm-ir"), true)
            } else {
                (engine.emit.as_ref(), engine.emit.as_ref().map(|v| &**v), false)
            }
        } else {
            (None, None, false)
        }
    };

    if let Some(rustc_emit) = rustc_emit {
        let mut new_command = CommandPrototype::new(command.get_type().clone()).unwrap();

        for arg in command.get_args().iter().filter(|a| !a.to_str().unwrap().starts_with("--emit")) {
            new_command.arg(arg);
        }

        for (key, val) in command.get_envs().iter() {
            new_command.env(&key[..], val.as_ref().unwrap());
        }

        new_command.cwd(command.get_cwd().clone());

        new_command.arg("--emit").arg(&format!("dep-info,{}", rustc_emit));

        if transform && is_binary && !is_build {
            new_command.arg("-C").arg("lto");
        }

        swap(&mut command, &mut new_command);
    }

    if let Some(ref sysroot) = engine.sysroot {
        if !is_build {
            command.arg("--sysroot").arg(&sysroot);
        }
    }

    let output = try!(do_exec(command.into_process_builder(), with_output));
    let ll_output_file = PathBuf::new(&format!("{}/{}.ll", out_dir, crate_name));

    if transform {
        llvm35_transform(engine.opt.as_ref().map(|v| &**v).unwrap_or(&Path::new("opt")), &*ll_output_file).unwrap();
    }

    match emit {
        Some(ref emit) if emit.starts_with("em-") => {
            let extension = match &emit[..] {
                "em-html" => "html",
                "em-js" => "js",
                _ => panic!("unsupported emscripten emit type"),
            };
            let mut process = util::process(engine.emcc.as_ref().unwrap_or(&PathBuf::new("emcc"))).unwrap();
            process.arg(&ll_output_file)
                .arg("-lGL").arg("-lSDL").arg("-s").arg("USE_SDL=2")
                .arg("-o").arg(&format!("{}/{}.{}", out_dir, crate_name, extension));
            do_exec(process, with_output)
        },
        _ => Ok(output),
    }
}

fn do_exec(process: ProcessBuilder, with_output: bool) -> Result<Option<Output>, ProcessError> {
    if with_output {
        process.exec_with_output().map(|o| Some(o))
    } else {
        process.exec().map(|_| None)
    }
}

fn llvm35_transform(opt: &Path, path: &Path) -> io::Result<()> {
    let input = try!(File::open(path));
    let input = BufReader::new(input);

    // Prepare LLVM optimization passes to remove llvm.assume and integer overflow checks
    let opt_path = self_exe_path().unwrap();
    let opt_path = Path::new(&opt_path);
    let mut opt = Command::new(opt);
    opt.arg(&format!("-load={}", opt_path.join("RemoveAssume.so").display()))
        .arg("-remove-assume")
        .arg("-globaldce")
        .arg("-S")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut opt = opt.spawn().unwrap();
    {
        let mut output = BufWriter::new(opt.stdin.as_mut().unwrap());

        // Run a string transform over the IR to fix metadata syntax for LLVM 3.5
        for line in input.lines() {
            let mut line = try!(line);
            let line = if line.starts_with("!") {
                line = line.replace("!", "metadata !");
                line = line.replace("distinct metadata", "metadata");
                &line[9..]
            } else {
                &line[..]
            };

            try!(output.write_all(line.as_bytes()));
            try!(output.write_all(&['\n' as u8]));
        }
    }

    drop(opt.stdin.take());

    {
        let mut output = try!(File::create(path));
        try!(copy(opt.stdout.as_mut().unwrap(), &mut output));
    }
    assert!(opt.wait().unwrap().success());

    Ok(())
}
