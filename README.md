# bf-jit
[Brainf*ck](https://esolangs.org/wiki/Brainfuck) interpreter written in Rust

## Spec
- 8-bit per cell (wrapping)
- allowing negative memory access
- tape size is fixed, abort when go out of range 
- JIT compilation (WIP, only for x64 Linux and MacOS)

## Build & Run

```
$ RUSTFLAGS="-C target-cpu=native" cargo run --release -- examples/mandelbrot.bf
```

### with JIT(WIP)

```
$ RUSTFLAGS="-C target-cpu=native" cargo run --release -- --with-jit examples/mandelbrot.bf
```
