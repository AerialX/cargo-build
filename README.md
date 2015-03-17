# cargo-build

Builds your Cargo projects for LLVM IR, alternate platforms, and Emscripten.

## Running

`cargo-build` runs just like `cargo build`, but provides a few additional options:

- `--sysroot SYSROOT` will pass the `--sysroot` flag on to `rustc`. See the
  Alternate Platforms section below for how this can be helpful.
- `--emit TYPE` allows you to specify the end format of the produced binary:
  - `llvm-ir`, `llvm-bc` tell `rustc` to produce LLVM IR or bitcode files instead
    of the usual linked binary. Enable LTO in your `Cargo.toml` in order to
    create standalone linked bitcode files that include all dependencies.
  - `llvm35-ir` is a special target that runs a hacky transform over the `llvm-ir`
    output that is mostly compatible with LLVM 3.5. This will force release mode
    and enable LTO.
  - `em-html`, `em-js` will produce Emscripten output. If you wish to have more
    control over its build flags, use `llvm35-ir` instead and call `emcc` on the
    output once the build is complete.
- `--opt OPT` provide the path to the `opt` executable that will be used to
  transform the IR. This should be LLVM 3.5.
- `--emcc EMCC` provide a path to the `emcc` executable when building Emscripten
  target formats.

**NOTE: Using `llvm35-ir` or any emit modes that depend on it (Emscripten) will
require a run of LLVM `opt`. Make sure you've built with `LLVM_PREFIX` as
described in the section below. The generated `Remove*.so` files must reside in
the same directory as the `cargo-build` binary.**

## Building

    cargo build

In order to build `cargo-build`, you need to use the same rust version as Cargo.
[multirust](https://github.com/brson/multirust) makes this easy, the version
to install is referenced in `src/rustversion.txt`. Alternatively, fix your
local Cargo for the latest rust (or find a relevant pull request) and override
it with `.cargo/config`

When building, provide a `LLVM_PREFIX` environment variable to the location of
an LLVM 3.5 install prefix if you intent on building for targets like Emscripten.

## Alternate Platforms

The `--sysroot` flag documented above can be used to provide an alternate `std`
and related crates to an application. This is useful for embedded targets and
others that may not actually be an LLVM target supported by rust, or to allow
for LLVM IR output that will be transformed later.

A lightweight `std` is provided in [rust-rt-minimal](https://github.com/AerialX/rust-rt-minimal)
for use in these situations. It includes a modified standard library with
threads and unwinding disabled for platforms that don't need or support them.

    git clone https://github.com/AerialX/rust-rt-minimal.git
    cd rust-rt-minimal/
    TRIPLE=arch-target-triple
    cargo build --release --target $TRIPLE
    mkdir -p sysroot/lib/rustlib/$TRIPLE/lib
    cp target/$TRIPLE/release/deps/lib*.rlib sysroot/lib/rustlib/$TRIPLE/lib/

## Emscripten

Install the [`incoming` branch](http://kripken.github.io/emscripten-site/docs/tools_reference/emsdk.html#how-do-i-track-the-latest-emscripten-development-with-the-sdk)
of Emscripten.

To build a project for Emscripten, you must first compile `std` as described
above. Use the `i386-unknown-emscripten` triple, which is provided as a
flexible target JSON in the rust-rt-minimal repo. Release mode must be used
due to metadata compatibility issues with LLVM 3.5.

Then build `cargo-build` as described above, making sure that you use the
`LLVM_PREFIX` environment variable to include the optimization passes.

Once that is set up, compiling an emscripten project is simply:

    cargo-build --sysroot path/to/sysroot --target i386-unknown-emscripten --emit em-html

See [here](https://github.com/AerialX/rust-emscripten-example) for a sample.

**NOTE: rustc currently [miscompiles some struct field accesses](https://github.com/rust-lang/rust/issues/23431)
in a way that may result in incorrect code execution with Emscripten.**

## See Also

[rust-emscripten-passes](https://github.com/epdtry/rust-emscripten-passes) are
used to transform `rustc`'s LLVM IR to 3.5 compatibility. It's automatically
pulled in by the build script when building `cargo-build`.

[cargo-emscripten](https://github.com/tomaka/cargo-emscripten) an older approach
that this program is loosely based off of.
