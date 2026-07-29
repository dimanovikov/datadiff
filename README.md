# datadiff

Semantic diff for structured data files — **JSON, YAML, CSV, TOML, XML**.

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
datadiff <old> <new> [--key <field>] [--format <json|yaml|csv|toml|xml>]
         [--show-unchanged] [--no-color] [--output <text|json>]
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

Machine-readable output for tooling (`--output json`):

```sh
$ datadiff examples/k8s-old.yaml examples/k8s-new.yaml --output json
{
  "changes": [
    { "type": "modified", "path": "spec.replicas", "old": 3, "new": 5 }
  ],
  "summary": { "added": 0, "removed": 0, "modified": 1, "total": 1 }
}
```

### Patch mode

A diff produced with `--output json` can be applied to a file with the
`patch` subcommand. The patched document is printed to stdout as JSON:

```sh
$ datadiff old.yaml new.yaml --output json > changes.json
$ datadiff patch old.yaml changes.json > new.json
```

Changes are applied by data path (`users[id=4217].email`); a path that
cannot be resolved in the target file is an error (exit code 2) — nothing
is skipped silently. Known limitation: object keys containing `.`, `[` or
`]` cannot be patched, because such paths are not representable.

A JSON array of objects matched by key — pure reordering reports nothing:

```sh
$ datadiff users-old.json users-new.json --key id
0 changes (0 added, 0 removed, 0 modified)
```

XML follows the same tree diff; attributes are reported under `@name` and
element text under `$text` (quick-xml mapping), and the root element name
is dropped:

```sh
$ datadiff examples/config-old.xml examples/config-new.xml
~ port.$text: "8080" → "9090"
~ tls.@enabled: "false" → "true"
~ tls.@version: "1.2" → "1.3"
3 changes (0 added, 0 removed, 3 modified)
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
| `DATADIFF_OUTPUT` | `--output`       |
| `DATADIFF_EXIT_ZERO` | `--exit-zero` |
| `DATADIFF_FAIL_ON` | `--fail-on`     |

Priority: CLI flag > `.env` > built-in default.

### Use as a git diff driver

To make `git diff` show semantic changes for structured files, register
`datadiff` as an external diff driver. Git calls the driver with seven
arguments, so the command picks out the two temp files (`$2` and `$5`):

```sh
git config diff.datadiff.command 'f() { datadiff --exit-zero "$2" "$5"; }; f'
```

```gitattributes
# .gitattributes
*.json diff=datadiff
*.yaml diff=datadiff
*.yml  diff=datadiff
```

`--exit-zero` is required: git treats a non-zero exit from the driver as a
failure, while `datadiff` normally exits 1 when differences are found.

### Format conversion

All supported formats parse into the same data tree, so files can be
converted between them (the output format comes from the extension):

```sh
$ datadiff convert config.yaml config.json
$ datadiff convert rows.json rows.csv
```

Limits: TOML needs an object at the document root (and no nulls), CSV needs
an array of flat objects, and writing XML is not supported. Such mismatches
are reported as errors (exit code 2).

### Risk policies for CI (`--fail-on`)

In CI you often want the gate to fail only on *risky* changes, not on
cosmetic ones. `--fail-on` takes path patterns (repeatable or
comma-separated); the exit code is 1 only when a change path matches:

```sh
$ datadiff old.yaml new.yaml --fail-on spec.replicas,*.image
~ spec.replicas: 3 → 5
~ spec.template.labels.team: "a" → "b"
2 changes (0 added, 0 removed, 2 modified)   # exit code 1: replicas matched
```

A pattern matches when it equals the path, is a dot-segment prefix of it
(`spec` matches `spec.replicas`), or uses `*` globs (`*.image`). Changes
that match nothing still print but exit 0.

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

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.
