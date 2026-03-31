---
name: pwalk-sdk-vault
description: Use when working outside polkadot-sdk but you need to consult polkadot-sdk code. This skill uses the local pwalk-generated Obsidian vault to find the right SDK package note, narrow the search space, and jump to the highest-signal SDK files instead of guessing or grepping the entire monorepo.
---

# Pwalk SDK Vault

Use when the task touches FRAME, Cumulus, XCM, proc-macros, or SDK build behavior and the relevant code lives in `polkadot-sdk`.

## Local paths

- SDK repo: `/Users/mmo/fun/pkdot/polkadot-sdk`
- Vault: `/Users/mmo/fun/pkdot/pwalk/vault/polkadot-sdk`
- Vault folders: `00-index/`, `10-packages/`
- pwalk binary: `cargo run --manifest-path /Users/mmo/fun/pkdot/pwalk/Cargo.toml -- --config /Users/mmo/fun/pkdot/pwalk/pwalk.toml`

## Constraint

The vault is a package-level index, not a symbol engine. Use it to narrow to the right package, then read SDK code directly.

## Workflow

### 1. Refresh only when needed

Resync only if a note is missing or the user asks. Don't resync on every task.

```bash
# check health
<pwalk> doctor --json
# resync if needed
<pwalk> vault sync --json
```

(`<pwalk>` = the full `cargo run` command above)

### 2. Find the right package note

By name:
```bash
rg --files /Users/mmo/fun/pkdot/pwalk/vault/polkadot-sdk/10-packages | rg 'pallet-balances|frame-support'
```

By concept:
```bash
rg -n 'TransactionExtension|OnUnbalanced|HoldReason' /Users/mmo/fun/pkdot/pwalk/vault/polkadot-sdk
```

### 3. Read the note before SDK code

Prioritize sections: `Identity` > `Description` > `Start Here` > `Important Files` > `Workspace Reverse Dependencies`. Treat frontmatter as truth, agent-generated sections as hints.

### 4. Jump to SDK code

Open the first 1-3 `Start Here` files. For usage examples, prefer packages from `Workspace Reverse Dependencies` over broad SDK grep.

### 5. Fall back

If no note fits: search vault contents with a narrower term, read 2-3 candidate notes, only then grep the SDK source tree.

## Output

State which note(s) you chose and which SDK files you opened. If the vault didn't help, say so before falling back.
