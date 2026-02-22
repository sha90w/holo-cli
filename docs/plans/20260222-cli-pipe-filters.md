# CLI Pipe Filters

## Overview

Add Unix-style pipe filter support to holo-cli so operators can chain output filters after
any show command, e.g.:

```
show route | include 10.0.0
show bgp summary | exclude Idle | count
show ospf neighbor | begin 10.1
```

The first token before `|` is a normal CLI command. Each `|`-separated token after it is a
**pipe command** looked up in a `PipeRegistry`. The pager (`less`) is appended automatically
at the tail of the chain when enabled. Callbacks write to `session.writer_mut()` instead of
calling `page_output()`, enabling true streaming through the pipe chain.

## Context (from discovery)

- **Branch**: `cli-pipe-filters`
- **Files involved**: all listed in Implementation Steps below
- **Key existing patterns**:
  - `Token` in `src/token.rs` — command tree nodes with `Action::Callback` or `Action::ConfigEdit`
  - `parse_command` in `src/parser.rs` — word-by-word arena walk returning `ParsedCommand`
  - `page_output(session, &str)` in `src/internal_commands.rs` — 12 call sites to refactor
  - `page_table(session, &Table)` — also used by `YangTableBuilder`
  - `Cli` struct in `src/main.rs` holds `commands: Commands` and `session: Session`
  - `CliCompleter` in `src/terminal.rs` uses `Arc<Mutex<Cli>>` for tab completion
- **No test infrastructure exists** — test tasks are omitted per project convention

## Development Approach

- Complete each task fully before moving to the next
- Make small, focused changes; build after each task (`cargo build`)
- Maintain backward compatibility — all existing commands continue to work unchanged
- No tests exist in this project; skip test steps

## Technical Details

### Pipe chain execution model

Chain is built **right-to-left** from parsed pipe stages. For each stage:

**External stage** (`PipeImpl::External`):
```
Command::new(binary)
  .args(base_args).args(user_args)
  .stdin(Stdio::piped())
  .stdout(Stdio::piped())
  .spawn()

thread: io::copy(child.stdout → next_write_end)
current_write_end = child.stdin
```

**Internal stage** (`PipeImpl::Internal`):
```
let (pipe_reader, pipe_writer) = std::io::pipe()
thread: filter_fn(BufReader(pipe_reader), next_write_end)
current_write_end = pipe_writer
```

**Pager** (when `use_pager=true`, always rightmost):
```
Command::new("less").args(["-F", "-X"])
  .stdin(Stdio::piped())
  .stdout(Stdio::inherit())
  .spawn()
current_write_end = pager.stdin
```

**Teardown** after callback returns:
1. Drop `session.writer` → closes leftmost write end → EOF propagates through chain
2. Join all relay threads
3. Wait for all child processes

### Key data structures

```rust
// src/pipe.rs
pub enum PipeImpl {
    External { binary: &'static str, base_args: Vec<&'static str> },
    Internal(fn(&mut dyn BufRead, &mut dyn Write) -> io::Result<()>),
}

pub struct PipeCommandDef {
    pub name:    &'static str,
    pub help:    &'static str,
    pub args:    &'static str,   // arg placeholder string, e.g. "PATTERN" or ""
    pub impl_:   PipeImpl,
}

pub struct PipeRegistry { commands: Vec<PipeCommandDef> }

// src/parser.rs
pub struct ParsedPipeStage { pub name: String, pub args: Vec<String> }
pub struct ParsedLine     { pub command: ParsedCommand, pub pipes: Vec<ParsedPipeStage> }
```

### Pipe command set (initial)

| Name      | Args    | Implementation              |
|-----------|---------|-----------------------------|
| `include` | PATTERN | internal Rust               |
| `exclude` | PATTERN | internal Rust               | 
| `count`   | —       | internal Rust               |
| `begin`   | PATTERN | internal Rust (skip until match) |
| `grep`    | ARGS    | call external grep with ARGS  |

### `pipeable` flag on commands

- `src/token.rs`: add `pipeable: bool` field to `Token`
- `src/internal_commands.xml`: add `pipeable="true"` on all `<token cmd="cmd_show_*">` nodes
- `src/token_xml.rs`: read the attribute (`None` → `false`)
- YANG-generated config tokens default to `pipeable=false`

## Progress Tracking

- Mark completed items with `[x]` immediately when done
- Add newly discovered tasks with ➕ prefix
- Document blockers with ⚠️ prefix

## Implementation Steps

---

### Task 1: Add `pipeable` flag to `Token`

**Files:**
- Modify: `src/token.rs`
- Modify: `src/token_xml.rs`
- Modify: `src/token_yang.rs`

- [ ] Add `pub pipeable: bool` field to `Token` struct in `src/token.rs`
- [ ] Add `pipeable` parameter to `Token::new()` and update all call sites
- [ ] In `src/token_xml.rs`: read `pipeable="true"` attribute in `parse_tag_token`; default `false`
- [ ] In `src/token_yang.rs`: pass `pipeable: false` for all generated tokens
- [ ] `cargo build` — must compile cleanly before Task 2

---

### Task 2: Create `src/pipe.rs` — registry and data structures

**Files:**
- Create: `src/pipe.rs`
- Modify: `src/main.rs` (add `mod pipe;`)

- [ ] Define `PipeImpl` enum (`External { binary, base_args }`, `Internal(fn)`)
- [ ] Define `PipeCommandDef` struct (`name`, `help`, `args`, `impl_`)
- [ ] Define `PipeRegistry` struct with `Vec<PipeCommandDef>`
- [ ] Implement `PipeRegistry::new() -> Self`
- [ ] Implement `PipeRegistry::add_external(name, help, args, binary, base_args) -> Self`
- [ ] Implement `PipeRegistry::add_internal(name, help, args, fn) -> Self`
- [ ] Implement `PipeRegistry::get(&str) -> Option<&PipeCommandDef>`
- [ ] Implement `PipeRegistry::names() -> impl Iterator<Item=&str>` (for completion)
- [ ] Add `mod pipe;` to `src/main.rs`
- [ ] `cargo build` — must compile cleanly before Task 3

---

### Task 3: Add `writer` field to `Session`

**Files:**
- Modify: `src/session.rs`

- [ ] Add `writer: Box<dyn std::io::Write + Send>` field to `Session` struct
- [ ] Initialize to `Box::new(std::io::stdout())` in `Session::new()`
- [ ] Add `pub fn writer_mut(&mut self) -> &mut dyn std::io::Write`
- [ ] Add `pub fn set_writer(&mut self, w: Box<dyn std::io::Write + Send>)`
- [ ] `cargo build` — must compile cleanly before Task 4

---

### Task 4: Add `ParsedLine` and `parse_line` to parser

**Files:**
- Modify: `src/parser.rs`
- Modify: `src/error.rs` (add new `ParserError` variants if needed)

- [ ] Add `pub struct ParsedPipeStage { pub name: String, pub args: Vec<String> }`
- [ ] Add `pub struct ParsedLine { pub command: ParsedCommand, pub pipes: Vec<ParsedPipeStage> }`
- [ ] Add `ParserError::NotPipeable` variant (command does not allow pipes)
- [ ] Add `ParserError::UnknownPipeCommand(String)` variant
- [ ] Implement `parse_line(session, commands, pipe_registry, line) -> Result<ParsedLine, ParserError>`:
  - Strip `!` comments from the full line first
  - Split on ` | ` to get `[cli_part, pipe_part…]`
  - Normalize whitespace in `cli_part`; return `None`-equivalent if empty
  - Call existing `parse_command` on `cli_part`
  - If `pipes` non-empty and `!token.pipeable`, return `Err(NotPipeable)`
  - For each pipe part: first word = name (validate against registry, else `UnknownPipeCommand`), rest = args
  - Return `ParsedLine { command, pipes }`
- [ ] Update `ParserError` `Display` impl for the two new variants
- [ ] `cargo build` — must compile cleanly before Task 5

---

### Task 5: Implement `build_output_chain` in `src/pipe.rs`

**Files:**
- Modify: `src/pipe.rs`

- [ ] Define `OutputChain` struct holding: `writer: Box<dyn Write+Send>`, `Vec<Child>`, `Vec<JoinHandle<io::Result<()>>>`
- [ ] Implement `OutputChain::writer(self) -> Box<dyn Write+Send>` (takes ownership of writer field, leaving a no-op placeholder)
- [ ] Implement `Drop for OutputChain`: join threads, wait for children
- [ ] Implement `build_output_chain(stages: &[ParsedPipeStage], registry: &PipeRegistry, use_pager: bool) -> io::Result<OutputChain>`:
  - Start with `current_write_end`: if `use_pager` spawn pager (`less -F -X`) and use its `stdin`; else `Box::new(io::stdout())`
  - Iterate pipe stages in **reverse** order:
    - `External`: spawn process `.stdin(piped()).stdout(piped())`, spawn relay thread `io::copy(stdout → current_write_end)`, set `current_write_end = Box::new(child.stdin)`
    - `Internal`: create `std::io::pipe()`, spawn thread running `filter_fn(BufReader(reader), current_write_end)`, set `current_write_end = Box::new(writer)`
  - Return `OutputChain { writer: current_write_end, children, threads }`
- [ ] `cargo build` — must compile cleanly before Task 6

---

### Task 6: Update `Cli` struct and `enter_command` in `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] Add `pipe_registry: PipeRegistry` field to `Cli` struct
- [ ] Build and assign `PipeRegistry` in `Cli::new()` with initial commands: `include`, `exclude`, `count`, `begin`
- [ ] Pass `&self.pipe_registry` where needed
- [ ] In `enter_command`: replace `normalize_input_line` + `parse_command` pair with `parse_line`
- [ ] After parsing: if `parsed.pipes` is non-empty, call `build_output_chain`; set `session.writer`
- [ ] Call the callback (unchanged)
- [ ] After callback: drop `OutputChain` (triggers teardown); restore `session.writer` to stdout
- [ ] `cargo build` — must compile cleanly before Task 7

---

### Task 7: Refactor `internal_commands.rs` — remove `page_output`/`page_table`

**Files:**
- Modify: `src/internal_commands.rs`

- [ ] Delete `fn pager()`, `fn page_output()`, `fn page_table()` functions
- [ ] For each of the 12 `page_output(session, &data)` call sites: replace with
  `write!(session.writer_mut(), "{}", data)` (or `writeln!` as appropriate)
- [ ] For each `page_table(session, &table)` call site: replace with
  `table.print(session.writer_mut())?; writeln!(session.writer_mut())?;`
- [ ] Update `YangTableBuilder` render/build method to write to `session.writer_mut()` similarly
- [ ] Remove `use std::process::{Child, Command, Stdio}` if no longer used here
- [ ] `cargo build` — must compile cleanly before Task 8

---

### Task 8: Mark show commands as `pipeable` in XML

**Files:**
- Modify: `src/internal_commands.xml`

- [ ] Add `pipeable="true"` attribute to every `<token>` element with a `cmd="cmd_show_*"` attribute
- [ ] Verify `clear` commands and config commands do **not** get `pipeable="true"`
- [ ] `cargo build` — must compile cleanly before Task 9

---

### Task 9: Tab completion for pipe commands in `terminal.rs`

**Files:**
- Modify: `src/terminal.rs`

- [ ] In `CliCompleter::complete`: detect whether the current line contains ` | `
- [ ] If yes, extract the token after the last `|` (the partial pipe command name)
- [ ] Return completions from `cli.pipe_registry.names()` filtered by the partial word, skipping the normal arena completion path
- [ ] Ensure completion still works normally (no `|` in line) — no regression
- [ ] `cargo build` — must compile cleanly before Task 10

---

### Task 10: Final verification

- [ ] Manual smoke test: `show route | include <prefix>` produces filtered output
- [ ] Manual smoke test: `show bgp summary | exclude Idle | count` chain of two filters works
- [ ] Manual smoke test: `show ospf neighbor | begin <pattern>` internal filter works
- [ ] Manual smoke test: Tab completion after `|` shows pipe command names
- [ ] Manual smoke test: `clear bgp neighbor` without `|` still works (not pipeable)
- [ ] Manual smoke test: `show route | unknowncmd` gives a clear error message
- [ ] `cargo clippy` — no warnings
- [ ] `rustfmt --edition 2024 $(git ls-files '*.rs')` — no formatting changes

---

### Task 11: Update documentation

**Files:**
- Modify: `CLAUDE.md` (if new patterns warrant it)
- Move: this plan to `docs/plans/completed/`

- [ ] Update `CLAUDE.md` if any new architectural patterns were established
- [ ] `mkdir -p docs/plans/completed && mv docs/plans/20260222-cli-pipe-filters.md docs/plans/completed/`

## Post-Completion

**Manual verification:**
- Test with a live `holod` instance: confirm pipe output matches what the equivalent Unix shell pipe would produce
- Test pager behavior: `show route | include 10.0.0` should still page if output is long
- Test `--no-pager` flag: pager should not appear in chain even when pipes are present
