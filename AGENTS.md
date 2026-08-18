# AGENTS.md — datadiff

Инструкции для агентов и контрибьюторов. Контекст продукта — в
[PROJECT.md](PROJECT.md), пользовательская документация — в
[README.md](README.md).

## Стек

- Rust 2021, бинарный crate `datadiff`
- Зависимости: clap 4 (`derive`, `env`), serde/serde_json, serde_yml,
  toml, csv, quick-xml, anyhow, colored, dotenvy
- Тесты: интеграционные (`tests/cli_tests.rs`), запускают собранный
  бинарник через `CARGO_BIN_EXE_datadiff`
- CI: GitHub Actions (`.github/workflows/ci.yml`) — build + test на
  ubuntu / windows / macos
- Релизы: `.github/workflows/release.yml` — по тегу `v*` собирает бинарники
  (linux x86_64/arm64, macos arm64, windows msvc) и выкладывает в
  GitHub Releases; `install.sh` / `install.ps1` качают бинарник из
  последнего релиза. Intel Mac не собираем (раннеры macos-13 выведены
  из эксплуатации) — там `cargo install datadiff`
- Соседние репозитории: `cloudroad-io/datadiff-action` (GitHub Action
  для CI-гейтов, floating-тег `v1`) и `cloudroad-io/homebrew-datadiff`
  (Homebrew tap). При новом релизе datadiff обновить sha256 в формуле
  и при необходимости дефолтную версию в Action

## Сборка и запуск

```sh
cargo build
cargo run -- examples/k8s-old.yaml examples/k8s-new.yaml
cargo run -- examples/products-old.csv examples/products-new.csv --key id
```

### Нюанс на Windows-машине разработки

Тулчейн `stable-x86_64-pc-windows-gnu` из коробки **не собирает проект**:
`windows-sys` генерирует import-библиотеки через `dlltool.exe`, а тот не
может запустить ассемблер `as` (его нет в self-contained MinGW). MSVC
Build Tools 2019 на машине неполные (нет cl.exe/link.exe), MSVC-тулчейн
не помогает.

Решение (уже применено): `as.exe` и его DLL (из winlibs, лежат в
`../.tools/mingw-binutils/`) скопированы рядом с dlltool в
`~/.rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/`.
dlltool ищет `as` только рядом с собой, PATH ему не помогает.
После переустановки/обновления тулчейна копию нужно восстановить.

Дополнительно: при **чистой** (не закэшированной) сборке windows-sys
сам rustc не находит `dlltool.exe` (`error calling dlltool
'dlltool.exe': program not found`) — лечится добавлением той же
self-contained папки в `PATH` на время сборки.

## Тесты

```sh
cargo test        # все тесты
cargo clippy -- -D warnings
cargo fmt --check # до коммита
```

Тесты — интеграционные, чёрный ящик: пишут временные файлы, запускают
бинарник, проверяют stdout и exit code. Новое поведение CLI покрывать
именно такими тестами.

## Стиль кода

- Стандартный `rustfmt`, без кастомных правил
- Комментарии и doc-comments — на английском
- Минимальные изменения: багфикс не трогает соседний код, фича не тащит
  лишние абстракции и конфигурируемость «на будущее»
- Ошибки — через `anyhow::Context` с понятным сообщением (что за файл,
  что пошло не так)
- Новые зависимости — только при реальной необходимости, сначала проверить,
  что задача не решается имеющимися

## Что нельзя трогать без явного запроса

- **Exit codes 0/1/2** (нет diff / есть diff / ошибка) — публичный
  контракт для CI-гейтов и скриптов
- **Легенду вывода** `~` / `+` / `-` / `=` и формат строки сводки —
  на них завязаны тесты и пользовательские парсеры
- **Git-операции** (commit, push и т.п.) — только по явной просьбе
  пользователя
- **`Cargo.lock`** — коммитить, не удалять
- Папку **`examples/`** не удалять; новые примеры добавлять парами
  `*-old.*` / `*-new.*`

## Конфигурация через .env

Переменные `DATADIFF_*` (см. `.env.example`) подхватываются через
`dotenvy` + фичу `env` у clap. Приоритет: CLI-флаг > `.env` > дефолт.
При добавлении нового CLI-параметра с env-аналогом: обновить
`.env.example`, README и PROJECT.md.
