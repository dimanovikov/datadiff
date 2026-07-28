# datadiff

Semantic diff for structured data files — **JSON, YAML, CSV, TOML**.

`datadiff` understands the *structure* of your data instead of comparing files
line by line. Reordered keys, reformatting and rewrapped YAML produce no
noise; real changes are reported as **data paths**, not line numbers.

## The pain

Plain `diff` on structured files is noisy:

- Reorder keys in a JSON object or reformat a Kubernetes manifest — and every
  line "changed".
- Reorder a JSON array of objects (e.g. a list of users) — and you get a
  wall of false positives.
- In a CSV export, one changed price hides inside a full-file text diff.

`datadiff` parses both files into a data tree and compares *that*:

- Key order and formatting are ignored.
- Arrays of objects can be matched by a key field (`--key id`), so reordering
  is not a change.
- CSV is compared row by row via a key column, reported as
  `row id=4217, column price: 100 → 120` style entries.

## Installation

```sh
cargo install --path .
# or from a release binary: put `datadiff` on your PATH
```

## Usage

```sh
datadiff <old> <new> [--key <field>] [--format <json|yaml|csv|toml>]
         [--show-unchanged] [--no-color]
```

The format is autodetected from the file extension; `--format` overrides it.

### Examples

Kubernetes manifests with reordered keys and one real change:

```sh
$ datadiff examples/k8s-old.yaml examples/k8s-new.yaml
~ spec.replicas: 3 → 5
1 changes (0 added, 0 removed, 1 modified)
```

A CSV price list, matched by the `id` column (reordering rows is free):

```sh
$ datadiff examples/products-old.csv examples/products-new.csv --key id
~ $[id=1001].price: 25.99 → 29.99
+ $[id=1006]: {"category":"electronics","id":1006,...}
- $[id=1003]: {"category":"home","id":1003,...}
3 changes (1 added, 1 removed, 1 modified)
```

A JSON array of objects matched by key — pure reordering reports nothing:

```sh
$ datadiff users-old.json users-new.json --key id
0 changes (0 added, 0 removed, 0 modified)
```

### Configuration via .env

Defaults can come from a `.env` file in the working directory
(see [`.env.example`](.env.example)):

| Variable        | Flag               |
| --------------- | ------------------ |
| `DATADIFF_KEY`  | `--key`            |
| `DATADIFF_FORMAT` | `--format`       |
| `DATADIFF_SHOW_UNCHANGED` | `--show-unchanged` |
| `DATADIFF_NO_COLOR` | `--no-color`   |

Priority: CLI flag > `.env` > built-in default.

### Output legend

- `~ path: old → new` — modified (yellow)
- `+ path: value` — added (green)
- `- path: value` — removed (red)
- `= path` — unchanged (grey, only with `--show-unchanged`)

Paths use dot notation (`services.web.replicas`); array elements are
`users[3].email` by index, or `users[id=4217].email` when matched by `--key`.
Scalar values are rendered in JSON representation.

### Exit codes

- `0` — no differences
- `1` — differences found
- `2` — error (file not found, invalid format)

The `0`/`1` split makes `datadiff` drop-in usable in CI gates and scripts.

## Roadmap

- **Git difftool driver** — `git difftool` / `.gitattributes` integration so
  `git diff` on JSON/YAML shows semantic changes.
- **Risk policies for CI** — fail only on high-risk changes (e.g. image tag or
  replica count in a manifest), ignore cosmetic ones.
- **XML support** — one more format behind the same tree diff.
- Machine-readable output (`--output json`) for tooling.

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.
