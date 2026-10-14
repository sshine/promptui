# promptui

A TUI widget collection for [Ratatui][ratatui], modelled on the look and feel of OpenCode's CLI.

[ratatui]: https://ratatui.rs

## Widgets

| Widget | Purpose |
| --- | --- |
| `agent_overview` | Summary panel for a running agent |
| `agent_scope` | Nested agent context with focus handling |
| `border_box` | Rounded-border container |
| `breadcrumb` | Path-style navigation trail |
| `message_list` | Scrollable chat transcript |
| `notice` | Inline info, warning and error callouts |
| `padding` | Inset wrapper for any widget |
| `questionnaire` | Multi-question prompt flow |
| `scroll_box` | Viewport with scroll state |
| `scrollbar` | Standalone scrollbar indicator |
| `spinner` | Activity indicator |
| `status_bar` | Bottom bar with model, latency and hints |
| `streaming_text` | Character-by-character text reveal |
| `text_field` | Single-line input with cursor |
| `todo_list` | Checklist with progress |
| `truncated_block` | Height-capped block with overflow marker |
| `word_wrap` | Unicode-aware line breaking |

Widgets share a `theme` for colors and an `input`/`focus` layer for key routing.

## Examples

```
cargo run -p promptui-examples --bin chat
cargo run -p promptui-examples --bin demo2
cargo run -p promptui-examples --bin hello
```

## Development

The repository ships a Nix flake with the toolchain, formatters and hooks:

```
nix develop
just --list
```

## License

MIT OR Apache-2.0.
