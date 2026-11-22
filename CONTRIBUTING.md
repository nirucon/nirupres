# Contributing

NIRUPRES is maintained as a personal application. Issues and constructive
feedback are welcome. Please open an issue before proposing substantial
architectural changes. Do not submit unrelated formatting changes.

Never commit user presentations, local preferences, secrets, caches or
generated build artifacts.

## Running the tests

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
./target/release/nirupres --self-test-fit
sh tests/xvfb.sh target/release/nirupres
```

`tests/xvfb.sh` runs in its own Xvfb with private XDG directories, so it never
reads or changes your decks, autosave or settings. It needs `Xvfb`,
`xdotool`, `xprop`, `python3` and the Noto fonts.
