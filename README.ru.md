# mxfind

[English](README.md) | Русский

```text
 __  __ __  __ _____ ___ _   _ ____
|  \/  |\ \/ /|  ___|_ _| \ | |  _ \
| |\/| | \  / | |_   | ||  \| | | | |
| |  | | /  \ |  _|  | || |\  | |_| |
|_|  |_|/_/\_\|_|   |___|_| \_|____/
```

**Matrix Federation Explorer**

`mxfind` - Rust CLI/TUI-инструмент для поиска публичных Matrix-комнат через public room directories разных homeserver'ов.

Проект помогает быстро собрать локальный индекс публичных комнат, искать по нему офлайн, получать компактный человекочитаемый вывод или полный JSON для скриптов.

## Содержание

- [Возможности](#возможности)
- [Как это работает](#как-это-работает)
- [Установка](#установка)
- [Быстрый старт](#быстрый-старт)
- [Команды](#команды)
- [Формат вывода](#формат-вывода)
- [Конфигурация](#конфигурация)
- [База данных](#база-данных)
- [TUI](#tui)
- [Ограничения Matrix federation](#ограничения-matrix-federation)
- [Диагностика](#диагностика)
- [Разработка](#разработка)
- [Roadmap](#roadmap)
- [Лицензия](#лицензия)

## Возможности

- Поиск публичных Matrix-комнат через Matrix Client-Server API `/_matrix/client/v3/publicRooms`.
- Индексация нескольких homeserver'ов за один запуск.
- Локальный SQLite-индекс для быстрых повторных запросов.
- Live search без локальной базы, если нужно быстро проверить public directories.
- Компактный CLI-вывод с alias, названием, количеством участников, сервером, topic preview и ссылкой `matrix.to`.
- JSON-вывод без обрезания данных для автоматизации.
- Просмотр полной карточки комнаты по room ID или canonical alias.
- Диагностика homeserver'ов через `mxfind status`.
- Live-статус сервера рядом с комнатами в человекочитаемом выводе.
- Экспериментальный TUI для интерактивного поиска, по умолчанию в live-режиме.
- Асинхронная сеть на Tokio/Reqwest.

## Как это работает

В Matrix нет единого глобального каталога всех публичных комнат federation. Вместо этого homeserver'ы могут отдавать свой public room directory.

`mxfind` работает в двух режимах:

1. **Local search** - ищет в SQLite-базе, которую создает команда `mxfind index`.
2. **Live search** - напрямую опрашивает настроенные homeserver'ы и фильтрует полученные комнаты на лету.

Рекомендуемый режим - локальный индекс:

```sh
mxfind index
mxfind search rust
```

Так поиск становится быстрым, повторяемым и не зависит от сетевых запросов при каждом запуске.

## Установка

### Требования

- Rust toolchain
- Cargo
- Доступ к сети для live search и индексирования

Проверить Rust:

```sh
rustc --version
cargo --version
```

### Сборка из исходников

```sh
git clone <repo-url>
cd mxfind
cargo build --release
```

Готовый бинарный файл:

```sh
./target/release/mxfind
```

### Локальная установка через Cargo

```sh
cargo install --path .
```

После этого `mxfind` будет доступен как обычная команда, если cargo bin directory находится в `PATH`.

### Запуск без установки

Во время разработки можно запускать через Cargo:

```sh
cargo run -- search rust
cargo run -- index
```

## Быстрый старт

1. Создайте или обновите локальный индекс:

```sh
mxfind index
```

Безопасно удалить устаревшие комнаты только с успешно просканированных серверов:

```sh
mxfind index --prune
```

2. Найдите комнаты:

```sh
mxfind search rust
```

3. Посмотрите полную информацию о комнате:

```sh
mxfind room '#rust:matrix.org'
```

4. Откройте интерактивный интерфейс с live-поиском:

```sh
mxfind tui
```

Для локального поиска через SQLite:

```sh
mxfind tui --local
mxfind --local tui
```

5. Проверьте статус homeserver'ов:

```sh
mxfind status
mxfind status --server matrix.org
```

Если запускать из исходников, добавьте `cargo run --`:

```sh
cargo run -- search rust
```

## Команды

### `mxfind`

Показывает banner, версию и короткую подсказку.

```sh
mxfind
```

### `mxfind index`

Опросить public room directories настроенных homeserver'ов и сохранить комнаты в SQLite.

```sh
mxfind index
```

Опции:

| Опция | Назначение |
| --- | --- |
| `--db <path>` | Использовать пользовательский путь к SQLite-базе. |
| `--config <path>` | Использовать пользовательский TOML-конфиг со списком серверов. |
| `-v, --verbose` | Показать skipped homeserver'ы и причины. |
| `--prune` | Удалить устаревшие комнаты только с homeserver'ов, которые были успешно просканированы. |

Пример:

```sh
mxfind index --config config.toml --db ./mxfind.sqlite
mxfind index --prune
```

По умолчанию `mxfind index` работает инкрементально: существующие комнаты обновляются, новые добавляются, старые не удаляются.

`mxfind index --prune` выполняет безопасную очистку: удаляет stale-комнаты только для homeserver'ов, которые успешно ответили в текущем запуске. Комнаты с skipped, timed-out, offline или restricted серверов сохраняются.

После завершения команда показывает:

- сколько серверов было просканировано;
- сколько серверов не ответило;
- сколько комнат было получено;
- сколько комнат было сохранено;
- сколько комнат было удалено, если включен `--prune`;
- путь к базе данных.

### `mxfind search <query>`

Искать комнаты по `room_id`, `canonical_alias`, `name` и `topic`.

```sh
mxfind search rust
```

По умолчанию команда использует локальную базу, если она существует. Если базы нет, `mxfind` переключается на live search.

Опции:

| Опция | Назначение |
| --- | --- |
| `-l, --limit <n>` | Максимальное количество результатов. По умолчанию `20`. |
| `--json` | Вывести полный JSON без обрезания topic. |
| `--local` | Принудительно искать только в локальной SQLite-базе. |
| `--live` | Принудительно выполнить live search по homeserver'ам. |
| `--db <path>` | Использовать пользовательский путь к SQLite-базе. |
| `--config <path>` | Использовать пользовательский TOML-конфиг для live search. |

Примеры:

```sh
mxfind search linux --limit 5
mxfind search matrix --local
mxfind search rust --live --config config.toml
mxfind search security --json
```

`--local` и `--live` нельзя использовать вместе.

### `mxfind room <identifier>`

Показать полную карточку одной комнаты из локального индекса.

`identifier` может быть:

- room ID, например `!abcdef:matrix.org`;
- canonical alias, например `#rust:matrix.org`.

```sh
mxfind room '#rust:matrix.org'
```

Опции:

| Опция | Назначение |
| --- | --- |
| `--json` | Вывести комнату как JSON. |
| `--db <path>` | Использовать пользовательский путь к SQLite-базе. |

В отличие от обычного `search`, команда `room` показывает полный `topic`.

### `mxfind status`

Проверить доступность Matrix homeserver'ов.

```sh
mxfind status
mxfind status --server matrix.org
mxfind status --json
```

Опции:

| Опция | Назначение |
| --- | --- |
| `--config <path>` | Использовать пользовательский TOML-конфиг со списком серверов. |
| `--server <name>` | Проверить один homeserver вместо списка из конфига. |
| `--json` | Вывести статусы серверов как JSON. |

Значения статусов:

| Статус | Значение |
| --- | --- |
| `online` | `publicRooms?limit=1` доступен. |
| `restricted` | Сервер доступен, но public rooms требуют авторизацию или запрещены. |
| `offline` | Таймаут или ошибка подключения. |
| `unknown` | Сервер вернул неожиданный ответ. |

### `mxfind tui`

Открыть экспериментальный терминальный интерфейс. По умолчанию TUI ищет live через public directories homeserver'ов.

```sh
mxfind tui
```

Опции:

| Опция | Назначение |
| --- | --- |
| `--db <path>` | Использовать пользовательский путь к SQLite-базе для локального TUI-поиска. |
| `--config <path>` | Использовать пользовательский TOML-конфиг для блока статусов серверов. |
| `--local` | Искать комнаты в локальной SQLite-базе вместо live homeserver'ов. |

Локальная база нужна только для режима `mxfind tui --local`. Перед локальным запуском выполните:

```sh
mxfind index
```

## Формат вывода

Обычный `search` выводит компактные карточки:

```text
Searching for: rust
Found 2 matching rooms
[1] #rust:matrix.org
    Name:    Rust
    Members: 12000
    Server:  matrix.org
    Status:  online
    Topic:   Rust programming language community
    Link:    https://matrix.to/#/#rust:matrix.org
```

Особенности человекочитаемого вывода:

- `topic` нормализуется в одну строку;
- переносы строк, табы и множественные пробелы заменяются одним пробелом;
- `topic` обрезается до короткого preview;
- `name`, `room_id` и `canonical_alias` не обрезаются;
- ссылка `matrix.to` строится из canonical alias, если он есть, иначе из room ID.

Человекочитаемый `search` также выполняет live-проверку статуса homeserver'ов из результатов и показывает статус рядом с каждой комнатой. JSON-вывод сохраняет исходную схему комнаты для совместимости со скриптами и не добавляет live-статус.

JSON-вывод не обрезает данные:

```sh
mxfind search rust --json
```

Это удобно для `jq`, shell-скриптов и интеграций:

```sh
mxfind search rust --json | jq '.[].canonical_alias'
```

## Конфигурация

Конфиг задает список homeserver'ов, которые нужно опрашивать.

Путь по умолчанию:

```text
~/.config/mxfind/config.toml
```

Пример:

```toml
servers = ["matrix.org", "envs.net", "tchncs.de"]
```

Можно передать конфиг явно:

```sh
mxfind index --config config.toml
mxfind search rust --live --config config.toml
```

Если конфиг не найден, используется встроенный список серверов.

## База данных

По умолчанию SQLite-база хранится здесь:

```text
~/.local/share/mxfind/mxfind.sqlite
```

Путь можно переопределить:

```sh
mxfind index --db ./mxfind.sqlite
mxfind index --prune --db ./mxfind.sqlite
mxfind search rust --db ./mxfind.sqlite
mxfind room '#rust:matrix.org' --db ./mxfind.sqlite
mxfind tui --db ./mxfind.sqlite
```

В базе сохраняются метаданные публичных комнат:

- room ID;
- canonical alias;
- name;
- topic;
- количество участников;
- homeserver, с которого комната была обнаружена;
- время первого обнаружения;
- время последнего обновления.

Данные не обрезаются перед сохранением. Обрезание topic применяется только к обычному человекочитаемому `search` output.

Поведение индексации:

- `mxfind index` инкрементальный и не удаляет существующие комнаты.
- `mxfind index --prune` удаляет устаревшие комнаты только для homeserver'ов, которые были успешно просканированы в текущем запуске.
- Prune не удаляет комнаты с серверов, которые упали, timed out или были skipped.

## TUI

TUI - экспериментальный терминальный интерфейс для live-поиска и локального SQLite-поиска.

По умолчанию TUI ищет live через public directories homeserver'ов. Используйте `mxfind tui --local` или `mxfind --local tui`, чтобы искать по локальной SQLite-базе.

В обоих режимах TUI может выполнять live-запросы для статусов homeserver'ов и проверки alias-статуса комнат.

Основные клавиши:

| Клавиша | Действие |
| --- | --- |
| Текстовый ввод | Ввод поискового запроса. |
| `Enter` | Выполнить поиск. |
| `Up` / `Down` | Перемещение по результатам или скролл деталей. |
| `Left` / `Right` | Переключение между списком результатов и деталями. |
| `PageUp` / `PageDown` | Быстрый скролл деталей. |
| `r` | Обновить статусы серверов, если поисковая строка пустая. |
| `Esc` | Выход. |
| `q` | Выход, если поисковая строка пустая. |

Блок `Servers` показывает настроенные homeserver'ы как `online`, `offline`, `restricted` или `unknown`. В результатах поиска и карточке комнаты также показывается live-статус сервера, с которого комната была найдена.

## Ограничения Matrix federation

Важно понимать границы инструмента:

- Matrix federation не имеет единого глобального public room search endpoint.
- `mxfind` видит только те комнаты, которые вернули выбранные homeserver'ы.
- Один homeserver может не знать о комнате, которую показывает другой homeserver.
- Некоторые серверы отключают public directory или требуют аутентификацию.
- Некоторые серверы могут отвечать медленно, возвращать ошибки или таймаутиться.
- Приватные комнаты не индексируются.

Поэтому `mxfind` - это explorer по доступным public directories, а не абсолютный каталог всей Matrix-сети.

## Диагностика

### `Local database not found. Run mxfind index first.`

Команда требует локальную базу, но она еще не создана.

Исправление:

```sh
mxfind index
```

Или передайте существующую базу:

```sh
mxfind search rust --local --db ./mxfind.sqlite
```

### Серверы падают во время index

Это нормально для federation: часть homeserver'ов может не отвечать или запрещать public directory.

`mxfind index` продолжит работу с остальными серверами и покажет количество failed servers.

### В live search мало результатов

Live search зависит от списка серверов в конфиге и от того, что они отдают через public directory.

Попробуйте расширить список:

```toml
servers = ["matrix.org", "envs.net", "tchncs.de", "kde.org", "gnome.org"]
```

## Разработка

Полезные команды:

```sh
cargo fmt --check
cargo clippy
cargo test
```

Локальные проверки поведения:

```sh
cargo run
cargo run -- --help
cargo run -- index
cargo run -- index --prune
cargo run -- search rust --limit 5
cargo run -- search rust --json
cargo run -- room '#rust:matrix.org'
cargo run -- status
cargo run -- tui
```

Структура основных модулей:

| Файл | Назначение |
| --- | --- |
| `src/main.rs` | Точка входа, маршрутизация команд. |
| `src/cli.rs` | Описание CLI-команд и опций через Clap. |
| `src/banner.rs` | CLI banner и branding. |
| `src/config.rs` | Загрузка TOML-конфига и серверы по умолчанию. |
| `src/matrix.rs` | Запросы к Matrix Client-Server API. |
| `src/db.rs` | SQLite schema, upsert, безопасный prune, local search и lookup комнаты. |
| `src/search.rs` | Фильтрация и дедупликация комнат. |
| `src/server_status.rs` | Use-case'ы статуса серверов и связывание комнат со статусом их сервера. |
| `src/output.rs` | Человекочитаемый вывод, JSON и topic preview. |
| `src/tui.rs` | Экспериментальный терминальный интерфейс. |
| `src/models.rs` | Общие модели данных. |

## Roadmap

Идеи для развития:

- полнотекстовый поиск через SQLite FTS5;
- фильтры `--server`, `--min-members`, `--has-alias`;
- команда `stats`;
- экспорт CSV;
- закладки и пользовательские теги;
- улучшенный TUI с поиском по мере ввода;

## Лицензия

MIT. См. [LICENSE](LICENSE).
