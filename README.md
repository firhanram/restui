# restui

A lightweight terminal UI for building and testing HTTP requests. Built with Rust, Ratatui, and Reqwest.

## Features

- GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS requests
- Enabled/disabled query parameters with URL encoding
- Raw JSON bodies for POST, PUT, and PATCH requests
- Pretty-printed, syntax-highlighted JSON responses with keyboard scrolling and clipboard copy
- Persistent workspaces and saved requests
- Postman v2/v2.1 collection import, including nested folders, query parameters, and raw bodies
- Asynchronous requests that keep the terminal interface responsive

## Run

Requires a Rust toolchain with Cargo.

```sh
cargo run --release
```

Install the binary locally:

```sh
cargo install --path .
restui
```

## Keyboard controls

| Key | Action |
| --- | --- |
| `Ctrl+S` | Send the selected request |
| `Ctrl+N` | Create a request |
| `Ctrl+W` | Open workspace switcher |
| `Ctrl+O` / `i` | Import a collection JSON file |
| `Tab` / `Shift+Tab` | Move between request, editor, and response panels |
| `Enter` | Edit the focused URL, params, or body; activate the focused method |
| `←` / `→` | Change the focused HTTP method or horizontally scroll the response |
| `↑` / `↓` | Select requests or vertically scroll the response |
| `PageUp` / `PageDown` | Scroll the response by ten lines |
| `y` | Copy the formatted JSON response (or plain response body) |
| `r` | Rename the selected request |
| `q` / `Ctrl+Q` | Save and quit |

While editing a URL or dialog field, use `Enter` or `Esc` to finish. The body editor supports multiline input, arrow-key navigation, `Home`, `End`, and `Tab` indentation; press `Esc` to finish editing.

In the query parameter editor:

- `a` adds a parameter.
- `Enter` or `e` edits it.
- `Space` enables or disables it.
- `d` deletes it.

In the workspace switcher, use `↑`/`↓` and `Enter` to switch, or `n` to create a workspace.

## Collection import

Press `Ctrl+O` or `i`, enter the path to a Postman collection JSON export, and press `Enter`. (`Ctrl+I` is reserved because terminals encode it as `Tab`.) Requests are added to the active workspace. Nested folder names are preserved in request names. String and object URLs, query parameters (including disabled parameters), HTTP methods, and raw request bodies are imported.

Collection variables such as `{{baseUrl}}` are retained verbatim rather than resolved. Non-raw body modes are not converted.

## Storage

Workspaces are saved automatically in the platform application-data directory:

- macOS: `~/Library/Application Support/dev.restui.restui/workspaces.json`
- Linux: `$XDG_DATA_HOME/restui/workspaces.json` or `~/.local/share/restui/workspaces.json`
- Windows: `%APPDATA%\\restui\\restui\\data\\workspaces.json`

Set `RESTUI_DATA_DIR` to override the storage directory. Writes use a temporary file and atomic rename.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```
