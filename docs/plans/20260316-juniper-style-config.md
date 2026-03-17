# Juniper-Style Configuration Mode

## Overview
- Replace IOS-style implicit config navigation with Juniper-style `set`/`delete`/`edit` commands
- Config display changes from indented CLI commands to curly-brace `{ }` hierarchical format
- Adds `| display set` pipe filter for flat `set`-command output
- Two-line prompt: `[edit path]` on first line, `hostname#` on second
- `show` scoped to current edit point; `show | compare` for diff

## Context (from discovery)
- Files involved: `token.rs`, `token_yang.rs`, `token_xml.rs`, `parser.rs`, `session.rs`, `internal_commands.rs`, `internal_commands.xml`, `terminal.rs`
- YANG token tree already handles path resolution and tab completion — reused as sub-tree for `set`/`edit`/`delete`
- Current IOS-style: implicit navigation by typing container names, `no` for negation, indented CLI display
- No tests exist — changes validated manually against holod

## Design Decisions
1. **Full replacement** — no IOS-style backward compatibility
2. **Curly-brace display** default, `| display set` pipe filter for flat format
3. **`delete` only** — `no` keyword removed
4. **Relative-only `edit`** — must `up`/`top` first to reach siblings
5. **Scoped `show`** — displays subtree from current edit point
6. **Commands:** current set + `set`, `delete`, `edit`, `up`, `run`; remove `no`

## Development Approach
- **testing approach**: Regular (no test infrastructure exists)
- complete each task fully before moving to the next
- make small, focused changes
- no existing tests to break — validate manually against holod
- **CRITICAL: update this plan file when scope changes during implementation**
- run `cargo build` and `cargo clippy` after each task

## Progress Tracking
- mark completed items with `[x]` immediately when done
- add newly discovered tasks with + prefix
- document issues/blockers with !! prefix
- update plan if implementation deviates from original scope

## Implementation Steps

### Task 1: Add `subtree_root` to Token and update data structures

**Files:**
- Modify: `src/token.rs`

- [x] add `subtree_root: Option<indextree::NodeId>` field to `Token` struct
- [x] update `Token::new()` to initialize `subtree_root` as `None`
- [x] run `cargo build` — must compile before next task

### Task 2: Update XML command definitions

**Files:**
- Modify: `src/internal_commands.xml`

- [x] remove `no` token from `config-default` tree
- [x] add `set` token with `cmd="cmd_set"` to `config-default`
- [x] add `delete` token with `cmd="cmd_delete"` to `config-default`
- [x] add `edit` token with `cmd="cmd_edit"` to `config-default`
- [x] add `up` token with `cmd="cmd_up"` to `config-default`
- [x] add `run` token with `cmd="cmd_run"` to `config-default`
- [x] move `discard` and `validate` from `config` tree to `config-default`
- [x] remove `config` tree entirely (now empty)
- [x] replace `show` > `changes` with `show` > `compare` subcommand
- [x] run `cargo build` — must compile before next task

### Task 3: Register new callbacks in token_xml.rs

**Files:**
- Modify: `src/token_xml.rs`

- [x] add callback mappings for `cmd_set`, `cmd_delete`, `cmd_edit`, `cmd_up`, `cmd_run` in the name-to-function mapping
- [x] remove `no` from any callback handling if present
- [x] run `cargo build` — must compile before next task

### Task 4: Implement stub callbacks in internal_commands.rs

**Files:**
- Modify: `src/internal_commands.rs`

- [x] add `cmd_set()` stub — placeholder that prints "set not yet implemented"
- [x] add `cmd_delete()` stub — placeholder
- [x] add `cmd_edit()` stub — placeholder
- [x] add `cmd_up()` stub — placeholder
- [x] add `cmd_run()` stub — placeholder
- [x] rename `cmd_show_config_changes` to `cmd_show_config_compare` (or update reference)
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 5: Wire `subtree_root` for `set`/`edit`/`delete` tokens at startup

**Files:**
- Modify: `src/token_xml.rs` or `src/main.rs` (wherever command trees are assembled)
- Modify: `src/token.rs` (if `Commands` struct needs a method)

- [x] after both XML and YANG token trees are built, set `subtree_root` on `set`, `edit`, `delete` tokens to point at `config_root_yang`
- [x] verify the linkage works at startup (cargo run, enter configure mode)
- [x] run `cargo build` — must compile before next task

### Task 6: Two-phase parser — sub-tree delegation for `set`/`edit`/`delete`

**Files:**
- Modify: `src/parser.rs`

- [x] modify `parse_command()` / `parse_command_try()`: when a matched token has `subtree_root`, continue parsing remaining words against the YANG subtree starting from that root (adjusted by current session stack position)
- [x] remove YANG tokens from top-level `get_tokens()` in config mode — only XML commands appear at top level
- [x] handle argument collection across the two-phase boundary (YANG tokens still carry `argument` fields)
- [x] ensure `ParsedCommand` correctly carries the final YANG token's action (`Action::ConfigEdit`)
- [x] run `cargo build` — must compile before next task

### Task 7: Two-phase tab completion

**Files:**
- Modify: `src/parser.rs`
- Modify: `src/terminal.rs` (if completion logic lives here)

- [x] modify completion logic: when cursor is after `set `/`edit `/`delete `, offer YANG token children starting from current edit point
- [x] for `edit`, filter completions to only containers and lists (not leaves)
- [x] resolve current edit point's YANG token ID from session stack for relative completion
- [x] verify tab completion works: `set <TAB>` shows top-level YANG nodes, `set routing <TAB>` shows routing children
- [x] run `cargo build` — must compile before next task

### Task 8: Implement `cmd_set` and `cmd_delete` callbacks

**Files:**
- Modify: `src/internal_commands.rs`
- Modify: `src/session.rs` (if `edit_candidate` needs adjustments)

- [x] implement `cmd_set`: extract resolved YANG schema node + args from parsed command, call `edit_candidate(negate=false)` — do NOT push to navigation stack
- [x] implement `cmd_delete`: same as `cmd_set` but `edit_candidate(negate=true)`
- [x] ensure `edit_candidate()` builds full YANG XPath correctly using current stack position + resolved path
- [x] verify: `configure` → `set routing ...` modifies candidate without changing prompt
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 9: Implement `cmd_edit` and `cmd_up` callbacks

**Files:**
- Modify: `src/internal_commands.rs`
- Modify: `src/session.rs`

- [x] implement `cmd_edit`: resolve YANG path, push one or more CommandNodes onto stack, create structural nodes in candidate DataTree
- [x] implement `cmd_up`: pop one CommandNode from stack (noop if at root)
- [x] modify `cmd_exit_config`: if stack is non-empty, act as `up`; if empty, exit config mode (already works — `mode_config_exit` pops or exits)
- [x] verify: `edit routing bgp` pushes multiple levels, prompt updates, `up` pops one level
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 10: Two-line prompt

**Files:**
- Modify: `src/terminal.rs`
- Modify: `src/session.rs` (prompt generation)

- [x] change prompt generation: first line `[edit]` or `[edit routing bgp]` based on stack
- [x] second line: `hostname#` (or `hostname>` for operational mode — keep existing)
- [x] ensure reedline handles multi-line prompt correctly (using `\n` in prompt string)
- [x] verify prompt displays correctly in terminal
- [x] run `cargo build` — must compile before next task

### Task 11: Curly-brace config display

**Files:**
- Modify: `src/internal_commands.rs`

- [x] rewrite `cmd_show_config_cmds()` to output curly-brace format: containers/lists → `name [keys] {` ... `}`, leaves → `name value;`, 4-space indent per level
- [x] scope output to current edit point using session stack's data_path
- [x] update `cmd_show_config()` to use the new formatter (unchanged — it already calls `cmd_show_config_cmds`)
- [x] verify: `show` in config mode displays curly-brace format
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 12: `| display set` pipe filter

**Files:**
- Modify: `src/internal_commands.rs`
- Modify: `src/internal_commands.xml` (if pipe commands defined in XML)
- Modify: `src/terminal.rs` or wherever pipe infrastructure lives

- [x] add `display set` as subcommand under `show candidate` and `show running` (not a global pipe — only available for config display)
- [x] implement flat `set`-command output: walks DataTree, outputs `set <path> <value>` per leaf
- [x] scope to current edit point — paths are relative when inside an edit section
- [x] wire in XML command definitions and `cmd_show_config` format handling
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 13: `show | compare` config diff

**Files:**
- Modify: `src/internal_commands.rs`

- [x] update `cmd_show_config_changes` (now `cmd_show_config_compare`): already uses `cmd_show_config_cmds` which now outputs curly-brace format
- [x] scope diff to current edit point
- [x] verify: `show compare` shows meaningful diff after `set` commands
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 14: Implement `cmd_run` callback

**Files:**
- Modify: `src/internal_commands.rs`
- Modify: `src/parser.rs` (may need to expose exec-tree parsing)

- ~~deferred~~ implement `cmd_run` — postponed to a future task

### Task 15: Remove IOS-style config navigation

**Files:**
- Modify: `src/parser.rs`
- Modify: `src/session.rs`

- [x] remove parser backtracking logic that auto-exits list entries on unmatched commands (no-op in config mode — `config_dflt_internal` has no ancestors)
- [x] remove implicit section entry (typing container name no longer navigates — YANG tokens not at top level)
- [x] clean up dead code from the old `no` command handling (removed negation check in `parse_command_try`)
- [x] verify: only `set`/`delete`/`edit` modify config; typing bare container names is rejected
- [x] run `cargo build` and `cargo clippy` — must pass before next task

### Task 16: Final integration verification

- [ ] verify full workflow: `configure` → `edit routing bgp` → `set as-number 65000` → `show` → `show | compare` → `show | display set` → `up` → `top` → `commit`
- [ ] verify `delete` removes config nodes
- [ ] verify `run show ...` works from config mode
- [ ] verify tab completion works for `set`, `edit`, `delete` at various edit depths
- [ ] verify two-line prompt displays correctly at all levels
- [ ] run `cargo clippy` with no warnings

## Technical Details

### Two-Phase Parsing
```
Input: "set routing bgp as-number 65000"

Phase 1 (XML tree):
  "set" → matches config-default token → has subtree_root

Phase 2 (YANG tree, from current edit point):
  "routing" → container token
  "bgp" → list token
  "as-number" → leaf token
  "65000" → String argument → collected as ("as-number", "65000")

Result: ParsedCommand {
  token_id: <yang leaf token>,
  action: ConfigEdit(as_number_snode),
  args: [("as-number", "65000")],
  negate: false,
  prefix: Set,  // new field to distinguish set/edit/delete
}
```

### Curly-Brace Display Algorithm
```
fn format_config(node, indent) -> String:
  for child in node.children():
    match child.schema().kind():
      Container → "{name} {\n" + format_config(child, indent+4) + "}\n"
      List → "{name} {keys} {\n" + format_config(child, indent+4) + "}\n"
      Leaf → "{name} {value};\n"
      LeafList → "{name} {value};\n"
```

### Display Set Algorithm
```
fn format_set(node, prefix) -> String:
  for child in node.children():
    path = prefix + " " + child.name() + keys_if_list
    match child.schema().kind():
      Container/List → format_set(child, path)
      Leaf/LeafList → "set {path} {value}\n"
```

### Navigation Stack Example
```
[edit]                          → stack: []
edit routing                    → stack: [routing]
edit bgp                        → stack: [routing, bgp]
set as-number 65000             → stack unchanged, candidate modified
up                              → stack: [routing]
top                             → stack: []
exit                            → operational mode
```

## Post-Completion

**Config file loading:** The existing `--file` / `read_config_file()` approach (line-by-line command replay) works unchanged with `set`-format files. Loading curly-brace format from disk (Juniper's `load merge`/`load override`) would need a dedicated parser (PEG crate would be a good fit) — defer to a future plan.

**Manual verification:**
- test against running holod instance with real YANG modules
- verify all protocol config paths work (BGP, OSPF, IS-IS, RIP, MPLS LDP)
- verify commit/discard/validate still work correctly with new command style
- verify hostname detection still works after commit
