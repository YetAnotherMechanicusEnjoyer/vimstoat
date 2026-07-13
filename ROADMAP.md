# VimStoat — Project Roadmap

> A lightweight, Vim-flavored TUI client for [Stoat.chat](https://stoat.chat)
> Built in Rust with Ratatui. Designed to feel like home for Vim users.

---

## Table of Contents

- [Vision](#vision)
- [Architecture Decision: Pure Rust vs Hybrid](#architecture-decision-pure-rust-vs-hybrid)
- [Authentication Strategy](#authentication-strategy)
- [Project Structure](#project-structure)
- [Vim Modal System](#vim-modal-system)
- [Keybinding Reference](#keybinding-reference)
- [Roadmap Phases](#roadmap-phases)
- [API Surface We Need](#api-surface-we-need)
- [Prior Art & Inspiration](#prior-art--inspiration)

---

## Vision

VimStoat is a terminal-first Stoat chat client that treats Vim keybindings as a first-class citizen — not an afterthought bolted onto a generic TUI. The goal is a client where a Vim user can navigate servers, channels, and messages entirely from muscle memory: `j`/`k` to scroll, `i` to compose, `Esc` to stop, `/` to search, `:q` to quit.

**What this is NOT:**

- A bot framework (that's what `stoat-rs` is for)
- A full reimplementation of the web client
- A project that needs every feature on day one

**What this IS:**

- A fast, keyboard-driven chat client
- Opinionated about UX — Vim's modal paradigm applied to chat
- A single static binary with zero runtime dependencies

---

## Architecture Decision: Pure Rust vs Hybrid

There's been discussion about two possible architectures. Here's an honest breakdown.

### Option A: Pure Rust (Recommended)

```
┌─────────────────────────────┐
│         vimstoat            │
│  ┌───────────┐ ┌──────────┐ │
│  │  Ratatui  │ │  reqwest  │ │
│  │  (TUI)    │ │  (HTTP)   │ │
│  └───────────┘ └──────────┘ │
│  ┌───────────┐ ┌──────────┐ │
│  │ crossterm │ │ tungstenite│ │
│  │  (input)  │ │  (WS)    │ │
│  └───────────┘ └──────────┘ │
│         Single Binary        │
└─────────────────────────────┘
```

**The Stoat/Revolt REST API is simple.** Looking at the actual endpoints we need (listed below in [API Surface](#api-surface-we-need)), we're talking about ~15 REST calls that are all `GET`/`POST`/`PATCH`/`DELETE` with JSON bodies. This is trivial to implement with `reqwest`. The WebSocket protocol is a single connection that sends/receives JSON events. `tokio-tungstenite` handles this cleanly.

| Pros                                       | Cons                                                      |
| ------------------------------------------ | --------------------------------------------------------- |
| Single binary, zero runtime deps           | Must hand-write API types (or pull `revolt-models` crate) |
| No IPC overhead, no serialization boundary | WebSocket reconnection logic is on us                     |
| Simpler deployment (`cargo install`)       | No access to JS ecosystem libraries                       |
| Everything is type-safe end-to-end         | If API changes, we update manually                        |
| Tokio async works perfectly with ratatui   |                                                           |
| No Node.js/npm/Bun dependency for users    |                                                           |

**Why stoat-rs isn't the answer:** The SDK is bot-centric — it uses `X-Bot-Token`, wraps everything in a `Client::new(EventHandler).run()` pattern that assumes you're building a bot, and the WebSocket layer is tightly coupled to that. For a user-facing TUI, we'd fight the SDK at every turn. However, the `revolt-models` crate (which `stoat-rs` re-exports as `stoat-models`) contains all the API type definitions and is perfectly usable standalone.

### Option B: Rust TUI + TypeScript API Backend

```
┌──────────────┐     IPC      ┌────────────────┐
│  Rust TUI    │◄────────────►│  Node/Bun      │
│  (Ratatui)   │  JSON-RPC    │  (stoat-api)   │
│  (crossterm) │  over stdin/ │  (revolt.js)   │
│              │  stdout or   │                │
│              │  unix socket │                │
└──────────────┘              └────────────────┘
   Process A                     Process B
```

The idea: let TypeScript handle the Stoat API (since `revolt.js`/`stoat-api` are the best-maintained client libraries with full WebSocket support, caching, and state management built in), and let Rust handle the TUI rendering.

| Pros                                                           | Cons                                                               |
| -------------------------------------------------------------- | ------------------------------------------------------------------ |
| `revolt.js` has battle-tested WS handling, caching, reactivity | **Two processes** — must manage lifecycle, crashes, zombies        |
| API types are always in sync with upstream                     | Serialization overhead on every message/event                      |
| If Stoat API changes, npm update fixes it                      | Users need Node.js/Bun installed — kills "single binary" story     |
| Richer ecosystem for API edge cases                            | Debugging across process boundary is painful                       |
|                                                                | JSON-RPC or IPC protocol is a whole sub-project to design          |
|                                                                | Latency: every keypress → IPC → TS → API → response → IPC → render |
|                                                                | Massively more complex build/packaging/distribution                |

**A middle-ground variant** would be `napi-rs` (Rust as a native Node addon, running in-process). This eliminates the IPC overhead but still requires Node.js at runtime. It's great for Electron/Tauri apps but awkward for a pure TUI — you'd be embedding a Node runtime just for HTTP calls.

### Verdict

**Go pure Rust.** The API surface is small enough that the "better JS libraries" argument doesn't hold up against the massive complexity tax of a hybrid architecture. We'd spend more time building and debugging the IPC bridge than we would writing 15 HTTP endpoints in Rust. The `revolt-models` crate gives us the types for free, and `reqwest` + `tokio-tungstenite` cover our networking needs.

If `revolt-models` ever becomes unmaintained, we can generate Rust types from the OpenAPI spec that Stoat publishes via `stoatchat/javascript-client-api`.

---

## Authentication Strategy

Stoat officially recommends that third-party clients **do not handle usernames and passwords**. Users obtain their session token from the web client and paste it into VimStoat. This is the same approach used by other third-party Revolt/Stoat clients.

### How It Works

1. User logs into Stoat web client
2. Opens browser DevTools → Application → Local Storage
3. Copies their session token
4. Pastes it into VimStoat on first launch
5. VimStoat stores it securely in the OS keyring via `keyring-lib`
6. On subsequent launches, token is loaded from keyring automatically

### Token Validation

Currently we accept any string as a token. We need to validate it by calling `GET /users/@me` with the token as `X-Session-Token`. If it returns a `User` object, the token is valid. If it returns 401, we prompt again.

### Auth Headers

| Client Type  | Header            | Our Case    |
| ------------ | ----------------- | ----------- |
| Bot          | `X-Bot-Token`     | ❌ Not us   |
| User session | `X-Session-Token` | ✅ This one |

### Instance Configuration

The base URL should be configurable to support self-hosted instances:

- Default: `https://api.stoat.chat`
- Configurable via: `~/.config/vimstoat/config.toml` or `--instance` CLI flag

---

## Project Structure

The current codebase is 3 files. Here's where we need to go:

```
vimstoat/
├── Cargo.toml
├── README.md
├── ROADMAP.md              ← you are here
├── config.example.toml     ← example user config
│
└── src/
    ├── main.rs             ← entry point: init terminal, run event loop, restore
    ├── app.rs              ← root App struct, owns all state, dispatches actions
    ├── action.rs           ← Action enum: every possible state mutation
    ├── input.rs            ← (Mode, KeyEvent) → Vec<Action>, pending-key buffer
    ├── tui.rs              ← terminal init/restore, panic hooks, alternate screen
    ├── event.rs            ← async event source: keys, ticks, API events via mpsc
    ├── error.rs            ← AppError enum (thiserror), Result<T> alias
    ├── config.rs           ← instance URL, theme, keybind overrides
    │
    ├── api/
    │   ├── mod.rs
    │   ├── client.rs       ← StoatApi struct: thin reqwest wrapper
    │   ├── auth.rs         ← token validation, keyring read/write
    │   └── ws.rs           ← WebSocket connection, event stream
    │
    ├── state/
    │   ├── mod.rs
    │   ├── chat.rs         ← server/channel/message/user caches
    │   └── ui.rs           ← scroll offsets, selections, panel focus
    │
    └── components/
        ├── mod.rs          ← Component trait definition
        ├── login.rs        ← token input screen
        ├── server_list.rs  ← left sidebar
        ├── channel_list.rs ← channel panel
        ├── message_view.rs ← main message area (scrollable)
        ├── message_input.rs← compose bar (Insert mode target)
        ├── command_line.rs ← ":" command bar
        └── status_bar.rs  ← mode indicator + context info
```

### Core Design Pattern

**Component + Action Dispatch** (ratatui community best practice):

```
 Crossterm KeyEvent
       │
       ▼
 ┌─────────────┐
 │  input.rs   │  Pure function: (Mode, Key) → Actions
 └──────┬──────┘
        │ Vec<Action>
        ▼
 ┌─────────────┐
 │   app.rs    │  match action { ... } → mutate state
 └──────┬──────┘
        │ &AppState
        ▼
 ┌─────────────┐
 │ components/ │  Pure rendering: (&State, Rect) → Frame
 └─────────────┘
```

State flows down. Events flow up as Actions. No component directly mutates state. This keeps everything testable and predictable.

### Key Dependencies

| Crate                    | Purpose                  | Replaces               |
| ------------------------ | ------------------------ | ---------------------- |
| `ratatui`                | TUI framework            | (keep)                 |
| `crossterm`              | terminal backend & input | (implicit via ratatui) |
| `tokio`                  | async runtime            | (keep)                 |
| `reqwest` (json feature) | HTTP client              | `stoat-rs`             |
| `revolt-models`          | API type definitions     | `stoat-rs` re-exports  |
| `tokio-tungstenite`      | WebSocket client         | —                      |
| `keyring-lib`            | secure token storage     | (keep)                 |
| `thiserror`              | error types              | `Box<dyn Error>`       |
| `serde` / `serde_json`   | JSON serialization       | —                      |
| `directories`            | XDG config paths         | —                      |
| `toml`                   | config file parsing      | —                      |

---

## Vim Modal System

### Modes

```rust
enum Mode {
    Normal,   // Default. Navigate, scroll, select.
    Insert,   // Typing a message. Input goes to compose bar.
    Command,  // ":" prefix. Commands like :quit, :join, :help.
    Visual,   // Future. Select text or messages.
}
```

The mode is always visible in the status bar: `-- NORMAL --`, `-- INSERT --`, etc.

### Mode Transitions

```
                    ┌──────────┐
          ┌────i────│  NORMAL  │────:────┐
          │    a    │ (default)│         │
          │    o    └────┬─────┘         │
          ▼              │               ▼
    ┌──────────┐    v (future)    ┌───────────┐
    │  INSERT  │         │        │  COMMAND  │
    │          │         ▼        │           │
    └────┬─────┘   ┌──────────┐  └─────┬─────┘
         │         │  VISUAL  │        │
         │         └────┬─────┘        │
         │              │              │
         └──── Esc ─────┴──── Esc ─────┘
                    (back to Normal)
```

### Pending Key Buffer

Vim has multi-key commands: `gg`, `dd`, `yy`, `Ctrl+w h`. We need a small state machine:

```rust
struct PendingKey {
    keys: Vec<KeyEvent>,
    timeout: Duration,    // reset if no follow-up within ~500ms
}
```

When `g` is pressed in Normal mode, we buffer it and wait. If `g` comes again within the timeout → `Action::JumpToTop`. If timeout expires or a different key comes → flush buffer as individual keys.

---

## Keybinding Reference

### Normal Mode — Navigation & Actions

**Movement:**
| Key | Action |
|-----|--------|
| `j` / `↓` | Select next item (channel, message) |
| `k` / `↑` | Select previous item |
| `h` / `←` | Focus panel left (servers ← channels ← messages) |
| `l` / `→` | Focus panel right |
| `gg` | Jump to top of list |
| `G` | Jump to bottom (most recent) |
| `Ctrl+d` | Half-page scroll down |
| `Ctrl+u` | Half-page scroll up |
| `Ctrl+f` | Full page down |
| `Ctrl+b` | Full page up |
| `H` | Top of visible area |
| `M` | Middle of visible area |
| `L` | Bottom of visible area |

**Mode switching:**
| Key | Action |
|-----|--------|
| `i` | Enter Insert mode (focus input bar) |
| `I` | Enter Insert mode, cursor at start |
| `a` | Enter Insert mode, cursor after current pos |
| `A` | Enter Insert mode, cursor at end |
| `o` | Enter Insert mode, start new message |
| `:` | Enter Command mode |
| `/` | Search (enters Command mode with `/` prefix) |
| `v` | Enter Visual mode (future) |

**Actions on messages:**
| Key | Action |
|-----|--------|
| `Enter` | Open/select (enter channel, expand thread) |
| `r` | Reply to selected message |
| `e` | Edit message (if yours) |
| `dd` | Delete message (if yours, with confirmation) |
| `yy` | Copy message content to clipboard |
| `n` | Next search result |
| `N` | Previous search result |

**Window management:**
| Key | Action |
|-----|--------|
| `Tab` | Cycle focus to next panel |
| `Shift+Tab` | Cycle focus to previous panel |
| `Ctrl+w h` | Focus panel left |
| `Ctrl+w j` | Focus panel below |
| `Ctrl+w k` | Focus panel above |
| `Ctrl+w l` | Focus panel right |

**General:**
| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+c` | Quit |
| `Ctrl+l` | Force redraw |

### Insert Mode — Composing Messages

| Key               | Action                         |
| ----------------- | ------------------------------ |
| `Esc` / `Ctrl+[`  | Back to Normal mode            |
| `Enter`           | Send message                   |
| `Ctrl+c`          | Cancel, back to Normal         |
| `Backspace`       | Delete character before cursor |
| `Ctrl+w`          | Delete word before cursor      |
| `Ctrl+u`          | Clear entire input line        |
| `Home` / `Ctrl+a` | Cursor to start of input       |
| `End` / `Ctrl+e`  | Cursor to end of input         |
| `Ctrl+←`          | Move cursor one word left      |
| `Ctrl+→`          | Move cursor one word right     |
| All other chars   | Insert into message            |

### Command Mode — `:` Commands

| Command                 | Action                                |
| ----------------------- | ------------------------------------- |
| `:q` / `:quit`          | Quit the application                  |
| `:w`                    | Not applicable (but could save draft) |
| `:wq`                   | Send current draft and quit           |
| `:join <channel>`       | Switch to channel                     |
| `:server <name>`        | Switch to server                      |
| `:reply`                | Reply to selected message             |
| `:edit`                 | Edit selected message                 |
| `:delete`               | Delete selected message               |
| `:search <term>`        | Search messages in current channel    |
| `:set <option> <value>` | Change a setting                      |
| `:help`                 | Show keybinding help overlay          |
| `:mark <name>`          | Bookmark current position (future)    |
| `Esc`                   | Cancel, back to Normal                |
| `Enter`                 | Execute command                       |
| `Tab`                   | Autocomplete command/argument         |
| `↑` / `↓`               | Command history                       |

---

## Roadmap Phases

### Phase 0: Cleanup (Current → Foundation)

**Goal:** Restructure the existing code into the target architecture without adding features.

- [ ] Create `error.rs` with proper `AppError` enum (replace `Box<dyn Error>`)
- [ ] Create `tui.rs` (extract terminal init/restore from main)
- [ ] Create `action.rs` with initial `Action` enum
- [ ] Create `input.rs` — wire up mode-based key dispatch
- [ ] Create `event.rs` — async event channel
- [ ] Refactor `app.rs` — separate auth state from UI mode
- [ ] Move login UI to `components/login.rs`
- [ ] Create `components/status_bar.rs` with mode indicator
- [ ] Implement `Mode` enum and mode transitions (Normal/Insert/Command)
- [ ] Add `Cargo.toml` dependencies, remove `stoat-rs`
- [ ] Validate token on entry with `GET /users/@me`

**Deliverable:** App compiles, launches, shows mode in status bar, `i`/`Esc`/`:q` all work. Token is validated against the real API.

---

### Phase 1: See Your Chats

**Goal:** Connect to Stoat and display real data.

- [ ] Create `api/client.rs` — `StoatApi` struct wrapping `reqwest`
- [ ] Create `api/auth.rs` — token validation + keyring integration
- [ ] Create `state/chat.rs` — server/channel/message cache
- [ ] Create `state/ui.rs` — scroll positions, selected indices, focus
- [ ] `components/server_list.rs` — fetch and render server sidebar
- [ ] `components/channel_list.rs` — channels for selected server
- [ ] `components/message_view.rs` — scrollable message history
- [ ] `components/message_input.rs` — compose bar with Insert mode
- [ ] Wire up `j`/`k` navigation in all panels
- [ ] Wire up `h`/`l` panel focus switching
- [ ] Wire up `gg`/`G` jump to top/bottom
- [ ] Wire up `Ctrl+d`/`Ctrl+u` scrolling
- [ ] Implement message sending via Insert mode + Enter

**Deliverable:** You can launch VimStoat, see your servers and channels, read messages, navigate with Vim keys, and send messages.

---

### Phase 2: Real-Time

**Goal:** Messages appear live without manual refresh.

- [ ] Create `api/ws.rs` — WebSocket connection to Stoat events endpoint
- [ ] Handle `Authenticate` handshake with session token
- [ ] Handle incoming events: new messages, edits, deletes
- [ ] Handle typing indicators (show who's typing)
- [ ] Handle presence updates (online/offline/idle)
- [ ] Implement reconnection logic with exponential backoff
- [ ] Integrate WS events into the event channel (`event.rs`)
- [ ] Update message cache reactively on WS events
- [ ] Send typing indicator when user is composing

**Deliverable:** VimStoat feels alive — messages from other users appear instantly, typing indicators show, presence is visible.

---

### Phase 3: Power User Features

**Goal:** The features that make a Vim user grin.

- [ ] Message actions: reply (`r`), edit (`e`), delete (`dd`)
- [ ] Yank message content to clipboard (`yy`)
- [ ] Search messages (`/` and `:search`)
- [ ] `n`/`N` to navigate search results
- [ ] Command history in Command mode (↑/↓)
- [ ] Tab completion for commands and channel names
- [ ] `:help` overlay showing all keybindings
- [ ] Configurable keybindings via `config.toml`
- [ ] Configurable instance URL
- [ ] Theme/color scheme configuration
- [ ] Notification indicators (unread channels)
- [ ] Mark channels as read

**Deliverable:** VimStoat is genuinely usable as a daily driver for basic chat.

---

### Phase 4: Polish & Beyond

**Goal:** Go from "usable" to "I prefer this over the web client."

- [ ] Visual mode for selecting text/messages
- [ ] Multi-line message composition
- [ ] File/image upload support
- [ ] Image preview (sixel/kitty protocol for supported terminals)
- [ ] Emoji reactions
- [ ] User profiles and member lists
- [ ] Thread support
- [ ] DM conversations
- [ ] Marks and jump list (like Vim's `'a`, `Ctrl+o`, `Ctrl+i`)
- [ ] Registers (clipboard history)
- [ ] Plugin system (Lua scripting?)
- [ ] Man page and shell completions
- [ ] CI/CD: GitHub Actions for builds + releases
- [ ] AUR / Homebrew / Nix package

---

## API Surface We Need

This is every Stoat REST endpoint VimStoat needs, organized by phase. This is why writing our own client is tractable — it's only ~20 endpoints total.

### Phase 1 (Core)

| Method | Endpoint                  | Purpose                                 |
| ------ | ------------------------- | --------------------------------------- |
| `GET`  | `/`                       | Fetch instance config (WS URL, CDN URL) |
| `GET`  | `/users/@me`              | Validate token, get own user info       |
| `GET`  | `/users/{id}`             | Fetch user details                      |
| `GET`  | `/users/dms`              | Fetch DM channel list                   |
| `GET`  | `/servers/{id}`           | Fetch server details + channels         |
| `GET`  | `/channels/{id}`          | Fetch channel details                   |
| `GET`  | `/channels/{id}/messages` | Fetch message history (paginated)       |
| `POST` | `/channels/{id}/messages` | Send a message                          |

### Phase 2 (WebSocket)

| Event                | Direction       | Purpose                 |
| -------------------- | --------------- | ----------------------- |
| `Authenticate`       | Client → Server | Auth with session token |
| `Ready`              | Server → Client | Initial state dump      |
| `Message`            | Server → Client | New message received    |
| `MessageUpdate`      | Server → Client | Message edited          |
| `MessageDelete`      | Server → Client | Message deleted         |
| `ChannelStartTyping` | Both            | Typing indicators       |

### Phase 3 (Extended)

| Method   | Endpoint                                         | Purpose              |
| -------- | ------------------------------------------------ | -------------------- |
| `PATCH`  | `/channels/{id}/messages/{id}`                   | Edit own message     |
| `DELETE` | `/channels/{id}/messages/{id}`                   | Delete own message   |
| `GET`    | `/channels/{id}/messages/search`                 | Search messages      |
| `PUT`    | `/channels/{id}/ack/{id}`                        | Mark channel as read |
| `GET`    | `/users/{id}/profile`                            | User profile info    |
| `PUT`    | `/channels/{id}/messages/{id}/reactions/{emoji}` | React to message     |

### Auth Header (All Requests)

```
X-Session-Token: <user's session token>
```

---

## Prior Art & Inspiration

Projects and tools that inform VimStoat's design:

| Project                                                            | What We Learn From It                                                                                    |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| [WeeChat](https://weechat.org/)                                    | The gold standard for terminal chat. Buffer-based architecture, plugin system, keybinding customization. |
| [senpai](https://sr.ht/~delthas/senpai/)                           | IRC TUI in Go. Clean, minimal, good UX for a chat TUI.                                                   |
| [ncspot](https://github.com/hrkfdn/ncspot)                         | Spotify TUI in Rust (ratatui). Great reference for async API integration + Vim-like navigation.          |
| [lazygit](https://github.com/jesseduffield/lazygit)                | Git TUI. Excellent panel navigation and keybinding design.                                               |
| [Neovim](https://neovim.io/)                                       | The modal editing model we're emulating. Pending keys, operator-motion, modes.                           |
| [revolt.js](https://github.com/revoltchat/revolt.js)               | Reference for how the WebSocket protocol and caching should work.                                        |
| [ratatui component template](https://github.com/ratatui/templates) | The project structure and patterns we're following.                                                      |

---

## Contributing

This project is in early development. If you're interested in contributing:

1. Read this roadmap
2. Check the phase we're currently on
3. Pick an unchecked item
4. Open a PR

The architecture is designed so that components are independent — you can build `server_list.rs` without touching `message_view.rs`.

---

_Last updated: July 2025_
