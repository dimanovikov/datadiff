# datadiff-cli

npm distribution of [datadiff](https://github.com/dimanovikov/datadiff) —
semantic diff for JSON, YAML, CSV, TOML and XML.

```sh
npx datadiff-cli old.json new.json
npm install -g datadiff-cli
```

Installing fetches the release binary for your platform; the package itself
carries none. The npm name is `datadiff-cli` because `datadiff` is taken by an
unrelated package. Intel Macs have no prebuilt binary — use
`cargo install datadiff` there.

Publishing is manual: bump `version` to match the release tag, then
`npm publish` from this directory.
