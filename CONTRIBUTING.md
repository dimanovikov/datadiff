# Contributing to datadiff

Bug reports, ideas and pull requests are welcome. Issues labeled
[good first issue](https://github.com/dimanovikov/datadiff/labels/good%20first%20issue)
are small and self-contained, a good place to start.

## Reporting a bug

Open an [issue](https://github.com/dimanovikov/datadiff/issues/new/choose) with
the datadiff version (`datadiff --version`), the command you ran, two small
input files that show the problem, and what you expected instead.

## Building and testing

datadiff is a single Rust binary crate.

```sh
cargo build
cargo test                    # integration tests, they run the built binary
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The tests in `tests/cli_tests.rs` are black-box: they write files, run the
binary and check its output and exit code. New CLI behavior gets a test of
that kind.

## Pull requests

- Keep a change to one thing; a bug fix does not need to touch the code around it.
- Comments and doc comments are in English. Errors go through `anyhow::Context`
  and say which file and what went wrong.
- A new dependency needs a reason that the existing ones do not cover.
- A new option with a `DATADIFF_*` variable also goes into `.env.example` and
  the README.

Some things are a public contract that scripts and CI gates rely on, so they
change only after an issue that agrees on it:

- exit codes: 0 for no differences, 1 for differences, 2 for an error;
- the output legend (`~`, `+`, `-`, `=`) and the summary line.

## License

datadiff is dual-licensed under MIT or Apache-2.0. Unless you say otherwise, a
contribution you submit is licensed the same way, without any additional terms.
