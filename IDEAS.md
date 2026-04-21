# UI Component Ideas

Inspired by OpenCode's TUI and similar CLI tools (Claude Code, Warp, Zed terminal).

## Already built

- **Theme** — semantic color palette with OpenCode dark defaults
- **Input** — crossterm key event abstraction (Enter vs Newline, Ctrl combos, word nav)
- **Spinner** — animated spinner with preset styles (moon phases, breathing dot, wave ripple)
- **Padding** — uniform/symmetric/directional spacing utility
- **WordWrap** — Unicode-aware word-boundary wrapping with character fallback
- **Scrollbar** — vertical scrollbar with half-block sub-cell precision
- **ScrollBox** — bottom-aligned scrollable container with auto-hide scrollbar
- **MessageList** — chat message list with role indicators, word wrap, Scrollable trait
- **TextField** — multi-line word-wrapping text input with cursor management
- **StatusBar** — mode label, activity spinner, model name, CWD, token count

## Layout & Chrome

- **SplitPane** — resizable vertical/horizontal split (drag border or %-based)
- **TabBar** — switchable tab strip with labels, hotkeys, and active indicator
- **Dialog / Modal** — centered overlay with title, body, and action buttons
- **Toast / Notification** — timed popup anchored to a corner, auto-dismiss
- **Breadcrumb** — path-style navigation trail (e.g. project > file > function)
- **Panel** — titled bordered container with optional collapse/expand

## Content Display

- **MarkdownRenderer** — inline markdown to styled spans (bold, italic, code, links, headings, lists)
- **SyntaxHighlightedBlock** — fenced code block with language-aware token coloring
- **DiffView** — side-by-side or unified diff with add/remove/change highlighting
- **TreeView** — expandable/collapsible tree (file explorer, JSON, AST)
- **Table** — column-aligned data with optional sorting, selection, and scroll
- **LogStream** — append-only auto-scrolling log viewer with level-based coloring
- **ProgressBar** — determinate (percentage) and indeterminate (pulsing) variants
- **Sparkline** — inline mini line chart for token usage, latency, etc.

## Input & Selection

- **Autocomplete** — dropdown suggestions attached to a text field, fuzzy-matched
- **CommandPalette** — fuzzy-searchable action list (Ctrl+P / Ctrl+K style)
- **SelectList** — single-select list with j/k navigation and type-to-filter
- **MultiSelect / Checkbox** — toggle items on/off with space, confirm with Enter
- **ConfirmPrompt** — inline yes/no question with keyboard shortcuts
- **RadioGroup** — mutually exclusive option set
- **FilePicker** — tree-based file/directory chooser with preview

## Conversation & Agent

- **ToolCallBlock** — collapsible block showing tool name, args, result, and duration
- **ThinkingBlock** — expandable "thinking" section with streaming dots or content
- **StreamingText** — character-by-character or chunk-by-chunk text reveal
- **CodeApplyPreview** — before/after diff with accept/reject per hunk
- **CostTracker** — running token/cost display (input tokens, output tokens, cache hits)
- **AgentTimeline** — vertical timeline of agent steps with status icons

## Theming & Polish

- **Light theme** preset
- **User-defined theme** loading from TOML/JSON
- **FocusRing** — visible focus indicator that moves between widgets
- **AnimationSystem** — shared tick-based animation coordinator (easing, transitions)
- **Responsive layout** — adapt widget arrangement to terminal width/height breakpoints
