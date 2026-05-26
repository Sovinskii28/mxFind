# mxfind

[English](README.md) | Русский

`mxfind` — Rust CLI/TUI-инструмент для поиска публичных Matrix-комнат через public room directories homeserver'ов.

## Содержание

- [Возможности](#возможности)
- [Скриншоты](#скриншоты)
- [Установка](#установка)
- [Быстрый старт](#быстрый-старт)
- [Команды](#команды)
- [Примеры использования](#примеры-использования)
- [Конфигурация](#конфигурация)
- [Архитектура](#архитектура)
- [Ограничения](#ограничения)
- [Планы](#планы)
- [Разработка](#разработка)
- [Лицензия](#лицензия)

## Возможности

- Поиск публичных Matrix-комнат через Client-Server API `publicRooms`
- Поиск по нескольким настроенным homeserver'ам
- Локальный SQLite-индекс для быстрых офлайн/локальных запросов
- Просмотр деталей комнаты по room ID или canonical alias
- JSON-вывод для скриптов и автоматизации
- Экспериментальный терминальный интерфейс
- Асинхронная работа с сетью через Tokio и Reqwest

## Установка

### Сборка из исходников

```sh
git clone <repo-url>
cd mxfind
cargo build --release
```

Бинарный файл будет доступен здесь:

```sh
./target/release/mxfind
```

### Установка через Cargo

TODO: опубликовать `mxfind` на crates.io.

Для локальной разработки установите из репозитория:

```sh
cargo install --path .
```

### Готовые бинарные файлы

TODO: добавить заранее собранные release-бинарники.

## Быстрый старт

Обычный рабочий процесс:

1. Создать или обновить локальный SQLite-индекс.
2. Выполнить поиск по локальному индексу.
3. Посмотреть информацию о комнате.
4. При желании открыть экспериментальный TUI.

```sh
mxfind index
mxfind search rust
mxfind room '#rust:matrix.org'
mxfind tui
```

Сначала выполните `mxfind index`. Команда опросит настроенные homeserver'ы и сохранит метаданные публичных комнат в локальную SQLite-базу.

TUI тоже использует локальную SQLite-базу, поэтому он также ожидает, что `mxfind index` уже был выполнен.

Путь к базе данных по умолчанию:

```text
~/.local/share/mxfind/mxfind.sqlite
```

## Команды

| Команда | Назначение | Частые опции |
| --- | --- | --- |
| `mxfind search <query>` | Ищет комнаты. Использует локальную БД, если она существует; иначе переключается на live search. | `--local`, `--live`, `--json`, `--limit <n>`, `--db <path>`, `--config <path>` |
| `mxfind index` | Получает публичные комнаты с настроенных homeserver'ов и сохраняет их в SQLite. | `--db <path>`, `--config <path>` |
| `mxfind room <identifier>` | Показывает детали одной проиндексированной комнаты по room ID или canonical alias. | `--json`, `--db <path>` |
| `mxfind tui` | Открывает экспериментальный терминальный интерфейс на основе локального SQLite-индекса. | `--db <path>` |

## Примеры использования

Создать локальный индекс:

```sh
mxfind index
```

Искать с поведением по умолчанию:

```sh
mxfind search rust
```

Принудительно выполнить live search по public directories homeserver'ов:

```sh
mxfind search rust --live
```

Принудительно искать в локальной SQLite-базе:

```sh
mxfind search linux --local --limit 5
```

Вывести результаты поиска как JSON:

```sh
mxfind search rust --json
```

Использовать пользовательский конфиг:

```sh
mxfind search rust --config config.toml
```

Посмотреть одну комнату из локального индекса:

```sh
mxfind room '#rust:matrix.org'
```

Вывести одну комнату как JSON:

```sh
mxfind room '#rust:matrix.org' --json
```

Открыть экспериментальный TUI:

```sh
mxfind tui
```

## Конфигурация

Путь к конфигу по умолчанию:

```text
~/.config/mxfind/config.toml
```

Пример:

```toml
servers = ["matrix.org", "envs.net"]
```

Если конфиг не существует, `mxfind` использует встроенный список homeserver'ов по умолчанию.

Также можно явно передать файл конфигурации:

```sh
mxfind index --config config.toml
mxfind search rust --live --config config.toml
```

## Архитектура

У `mxfind` есть два режима поиска:

- **Live search** напрямую опрашивает выбранные homeserver'ы через Matrix Client-Server endpoint `/_matrix/client/v3/publicRooms`.
- **Локальный поиск** ищет в SQLite-индексе, созданном командой `mxfind index`.

Команда индексирования получает public room directories с настроенных homeserver'ов, удаляет дубликаты комнат и добавляет или обновляет записи в SQLite.

TUI намеренно работает только локально. Он ищет в SQLite-индексе и не выполняет сетевые запросы.

## Ограничения

Matrix не предоставляет единого глобального endpoint'а для поиска публичных комнат по всей federation.

`mxfind` ищет в public room directories, которые отдают выбранные homeserver'ы. Поэтому результаты зависят от настроенных серверов и от того, что эти серверы решают показывать.

Некоторые homeserver'ы могут:

- не отвечать
- завершаться по таймауту
- отключать или ограничивать публичный directory
- требовать аутентификацию для доступа к directory

Приватные комнаты не индексируются. Комнаты, которые не вернул опрошенный public directory, не будут видны в `mxfind`.

## Планы

- Улучшенный TUI
- Аналитика federation
- Статистика комнат
- Улучшения кэширования
- Инкрементальная индексация
- Полнотекстовый поиск
- Теги/категории

## Разработка

```sh
cargo fmt
cargo clippy
cargo test
```

Полезные локальные проверки:

```sh
cargo check
cargo run -- index
cargo run -- search rust
cargo run -- tui
```

## Лицензия

MIT placeholder.
