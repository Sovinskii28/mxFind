# Как работает поиск по слову из `search`

В `mxfind` слово, которое вводится в команду `search`, используется как обычная подстрока. Например:

```bash
mxfind search rust
```

Здесь `rust` - это поисковый запрос.

## Общий поток

1. Пользователь запускает `mxfind search <query>`.
2. Команда попадает в обработчик `Command::Search` в `src/main.rs`.
3. Программа решает, где искать:
   - в локальной SQLite-базе, если база существует или указан `--local`;
   - через live search по homeserver'ам, если указан `--live` или локальной базы нет.
4. Найденные комнаты выводятся обычным текстом или JSON, если указан `--json`.

## Откуда берутся комнаты

Комнаты берутся из публичных каталогов Matrix homeserver'ов. В коде это делает функция `fetch_public_rooms` из `src/matrix.rs`.

Для каждого сервера строится URL:

```rust
let url = format!("https://{server}/_matrix/client/v3/publicRooms");
```

Например, для `matrix.org` получится:

```text
https://matrix.org/_matrix/client/v3/publicRooms
```

Это Matrix endpoint, который возвращает публичные комнаты, известные конкретному homeserver'у.

Список серверов берётся из конфига. Если пользователь не указал свой конфиг, используется список по умолчанию из `src/config.rs`:

```rust
vec![
    "matrix.org".to_string(),
    "tchncs.de".to_string(),
    "midov.pl".to_string(),
    "matrix.tchncs.de".to_string(),
]
```

Можно передать свой TOML-конфиг через `--config`.

## Как комнаты попадают в локальную базу

Когда пользователь запускает:

```bash
mxfind index
```

программа:

1. Загружает список homeserver'ов.
2. Для каждого сервера вызывает `fetch_public_rooms`.
3. Получает список публичных комнат.
4. Убирает дубликаты через `dedup_rooms`.
5. Сохраняет комнаты в SQLite через `upsert_rooms`.

Таблица создаётся в `src/db.rs`:

```sql
CREATE TABLE IF NOT EXISTS rooms (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    room_id TEXT NOT NULL,
    canonical_alias TEXT,
    name TEXT,
    topic TEXT,
    num_joined_members INTEGER,
    server TEXT NOT NULL,
    discovered_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
)
```

То есть локальный `search` ищет не в интернете напрямую, а в таблице `rooms`, которую раньше заполнила команда `mxfind index`.

По умолчанию база лежит здесь:

```text
~/.local/share/mxfind/mxfind.sqlite
```

## Локальный поиск

Локальный поиск выполняется функцией `search_rooms` в `src/db.rs`.

Сначала программа пытается использовать SQLite FTS5-индекс `rooms_fts`.
Он строится поверх полей:

- `room_id`
- `canonical_alias`
- `name`
- `topic`

FTS-запрос разбивается на токены и ищет их как префиксы. Поэтому запрос:

```rust
mxfind search "#rust:matrix.org"
```

сможет найти комнату по alias, даже если в запросе есть символы вроде `#` и `:`.

Если FTS не дал результатов, включается fallback на обычный substring search. Для него запрос
оборачивается в SQL-шаблон:

```rust
let pattern = format!("%{}%", search_query);
```

Если пользователь ввёл `rust`, SQLite получает шаблон:

```text
%rust%
```

Символы `%` означают: совпадение может быть в любом месте строки. Поэтому запрос `rust` найдёт и `rust`, и `rust-lang`, и `matrix-rust`.

Поиск идёт по четырём полям комнаты:

- `room_id`
- `canonical_alias`
- `name`
- `topic`

В SQL это выглядит так:

```sql
WHERE
    room_id LIKE ? COLLATE NOCASE
    OR coalesce(canonical_alias, '') LIKE ? COLLATE NOCASE
    OR coalesce(name, '') LIKE ? COLLATE NOCASE
    OR coalesce(topic, '') LIKE ? COLLATE NOCASE
```

`COLLATE NOCASE` делает fallback-поиск независимым от регистра. `Rust`, `RUST` и `rust`
будут искаться одинаково.

Результаты сортируются по количеству участников:

```sql
ORDER BY coalesce(num_joined_members, 0) DESC
```

То есть комнаты с большим числом участников показываются выше. После этого применяется `LIMIT`, который задаёт максимальное количество результатов.

## Live search

Live search выполняется функцией `search_live_rooms` в `src/main.rs`.

В этом режиме программа:

1. Загружает список homeserver'ов из конфига.
2. Пытается выполнить серверный поиск через `generic_search_term` endpoint'а
   `/_matrix/client/v3/publicRooms`.
3. Убирает дубликаты через `dedup_rooms`.
4. Если серверный поиск недоступен или ничего не вернул, запрашивает публичные комнаты и
   фильтрует их в памяти через `filter_rooms` из `src/search.rs`.

Главное отличие от локального поиска: live search не берёт комнаты из SQLite. Он заново идёт к homeserver'ам, получает их public rooms и сразу фильтрует результат.

Фильтрация работает через функцию `room_matches`.

Она тоже приводит запрос к нижнему регистру:

```rust
let query = query.to_lowercase();
```

Потом проверяет, содержится ли запрос в одном из полей:

```rust
field_matches(&query, &room.room_id)
    || option_field_matches(&query, room.name.as_deref())
    || option_field_matches(&query, room.topic.as_deref())
    || option_field_matches(&query, room.canonical_alias.as_deref())
```

Сама проверка совпадения простая:

```rust
value.to_lowercase().contains(query)
```

То есть live search, как и локальный поиск, ищет подстроку без учёта регистра.

## TUI-поиск

В TUI пользователь вводит текст в строку `Search: ...`. Каждый символ добавляется в `state.query`.

Поиск запускается не автоматически, а по `Enter`:

```rust
state.results = search_rooms(pool, &state.query, TUI_RESULT_LIMIT).await?;
```

По умолчанию TUI работает в live-режиме и использует тот же live search, что и команда
`mxfind search --live`. Если запустить `mxfind tui --local`, TUI использует локальную
SQLite-базу и функцию `search_rooms`.

## Откуда берётся JSON

JSON не берётся из отдельного файла или отдельного API. Это просто другой формат вывода уже найденных комнат.

Флаг `--json` описан в `src/cli.rs`:

```rust
/// Print results as JSON.
#[arg(long)]
json: bool,
```

Когда пользователь запускает:

```bash
mxfind search rust --json
```

`clap` видит флаг `--json` и записывает в поле `json` значение `true`. Если флага нет, значение остаётся `false`.

Дальше в `src/main.rs` это значение передаётся в функцию вывода:

```rust
print_search_results(&matches, limit, json)?;
```

Внутри `print_search_results` выбирается формат:

```rust
if json {
    print_rooms_json(rooms, limit)
} else {
    println!("Found {} matching rooms", rooms.len());
    print_rooms(rooms, limit);
    Ok(())
}
```

Если `json == false`, программа печатает обычные карточки комнат через `print_rooms`.

Если `json == true`, программа вызывает `print_rooms_json` из `src/output.rs`:

```rust
pub fn print_rooms_json(rooms: &[Room], limit: usize) -> anyhow::Result<()> {
    let rooms = sorted_limited_rooms(rooms, limit);
    let json = serde_json::to_string_pretty(&rooms).context("failed to serialize rooms as JSON")?;

    println!("{json}");
    Ok(())
}
```

Здесь происходит главное:

1. `sorted_limited_rooms` сортирует найденные комнаты и применяет `limit`.
2. `serde_json::to_string_pretty(&rooms)` превращает список структур `Room` в красивый JSON.
3. `println!("{json}")` выводит этот JSON в терминал.

Структура `Room` находится в `src/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub room_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub canonical_alias: Option<String>,
    pub num_joined_members: Option<u64>,
    pub server: String,
}
```

JSON получается именно из этой структуры. Поля JSON совпадают с полями `Room`: `room_id`, `name`, `topic`, `canonical_alias`, `num_joined_members` и `server`.

## Важные детали

- Это не полнотекстовый поиск, а поиск подстроки.
- Поиск не учитывает регистр.
- Поиск не исправляет опечатки.
- Поиск не разбивает запрос на отдельные слова.
- В локальном режиме результаты сортируются по числу участников.
- В live search результат зависит от того, какие комнаты вернули настроенные homeserver'ы.
- Комната считается найденной, если запрос встретился хотя бы в одном из полей: `room_id`, `canonical_alias`, `name` или `topic`.
