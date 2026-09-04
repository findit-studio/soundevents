<div align="center">
<h1>soundevents-dataset</h1>
</div>
<div align="center">

Typed, zero-allocation Rust access to [Google's AudioSet](https://research.google.com/audioset/) sound-event taxonomy. Two views are available — the full 632-entry [ontology](https://github.com/audioset/ontology) and the 527-class rated label set used by released AudioSet models — both baked in at compile time as `&'static` data, with case-insensitive perfect-hash lookup.

[<img alt="github" src="https://img.shields.io/badge/github-findit--ai/soundevents-8da0cb?style=for-the-badge&logo=Github" height="22">][Github-url]
<img alt="LoC" src="https://img.shields.io/endpoint?url=https%3A%2F%2Fgist.githubusercontent.com%2Fal8n%2F327b2a8aef9003246e45c6e47fe63937%2Fraw%2Fsoundevents" height="22">
[<img alt="Build" src="https://img.shields.io/github/actions/workflow/status/findit-studio/soundevents/ci.yml?logo=Github-Actions&style=for-the-badge" height="22">][CI-url]
[<img alt="codecov" src="https://img.shields.io/codecov/c/gh/findit-studio/soundevents?style=for-the-badge&token=6R3QFWRWHL&logo=codecov" height="22">][codecov-url]

[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-soundevents--dataset-66c2a5?style=for-the-badge&labelColor=555555&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">][doc-url]
[<img alt="crates.io" src="https://img.shields.io/crates/v/soundevents-dataset?style=for-the-badge&logo=data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iaXNvLTg4NTktMSI/Pg0KPCEtLSBHZW5lcmF0b3I6IEFkb2JlIElsbHVzdHJhdG9yIDE5LjAuMCwgU1ZHIEV4cG9ydCBQbHVnLUluIC4gU1ZHIFZlcnNpb246IDYuMDAgQnVpbGQgMCkgIC0tPg0KPHN2ZyB2ZXJzaW9uPSIxLjEiIGlkPSJMYXllcl8xIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHhtbG5zOnhsaW5rPSJodHRwOi8vd3d3LnczLm9yZy8xOTk5L3hsaW5rIiB4PSIwcHgiIHk9IjBweCINCgkgdmlld0JveD0iMCAwIDUxMiA1MTIiIHhtbDpzcGFjZT0icHJlc2VydmUiPg0KPGc+DQoJPGc+DQoJCTxwYXRoIGQ9Ik0yNTYsMEwzMS41MjgsMTEyLjIzNnYyODcuNTI4TDI1Niw1MTJsMjI0LjQ3Mi0xMTIuMjM2VjExMi4yMzZMMjU2LDB6IE0yMzQuMjc3LDQ1Mi41NjRMNzQuOTc0LDM3Mi45MTNWMTYwLjgxDQoJCQlsMTU5LjMwMyw3OS42NTFWNDUyLjU2NHogTTEwMS44MjYsMTI1LjY2MkwyNTYsNDguNTc2bDE1NC4xNzQsNzcuMDg3TDI1NiwyMDIuNzQ5TDEwMS44MjYsMTI1LjY2MnogTTQzNy4wMjYsMzcyLjkxMw0KCQkJbC0xNTkuMzAzLDc5LjY1MVYyNDAuNDYxbDE1OS4zMDMtNzkuNjUxVjM3Mi45MTN6IiBmaWxsPSIjRkZGIi8+DQoJPC9nPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPGc+DQo8L2c+DQo8Zz4NCjwvZz4NCjxnPg0KPC9nPg0KPC9zdmc+DQo=" height="22">][crates-url]
[<img alt="crates.io" src="https://img.shields.io/crates/d/soundevents-dataset?color=critical&logo=data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBzdGFuZGFsb25lPSJubyI/PjwhRE9DVFlQRSBzdmcgUFVCTElDICItLy9XM0MvL0RURCBTVkcgMS4xLy9FTiIgImh0dHA6Ly93d3cudzMub3JnL0dyYXBoaWNzL1NWRy8xLjEvRFREL3N2ZzExLmR0ZCI+PHN2ZyB0PSIxNjQ1MTE3MzMyOTU5IiBjbGFzcz0iaWNvbiIgdmlld0JveD0iMCAwIDEwMjQgMTAyNCIgdmVyc2lvbj0iMS4xIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHAtaWQ9IjM0MjEiIGRhdGEtc3BtLWFuY2hvci1pZD0iYTMxM3guNzc4MTA2OS4wLmkzIiB3aWR0aD0iNDgiIGhlaWdodD0iNDgiIHhtbG5zOnhsaW5rPSJodHRwOi8vd3d3LnczLm9yZy8xOTk5L3hsaW5rIj48ZGVmcz48c3R5bGUgdHlwZT0idGV4dC9jc3MiPjwvc3R5bGU+PC9kZWZzPjxwYXRoIGQ9Ik00NjkuMzEyIDU3MC4yNHYtMjU2aDg1LjM3NnYyNTZoMTI4TDUxMiA3NTYuMjg4IDM0MS4zMTIgNTcwLjI0aDEyOHpNMTAyNCA2NDAuMTI4QzEwMjQgNzgyLjkxMiA5MTkuODcyIDg5NiA3ODcuNjQ4IDg5NmgtNTEyQzEyMy45MDQgODk2IDAgNzYxLjYgMCA1OTcuNTA0IDAgNDUxLjk2OCA5NC42NTYgMzMxLjUyIDIyNi40MzIgMzAyLjk3NiAyODQuMTYgMTk1LjQ1NiAzOTEuODA4IDEyOCA1MTIgMTI4YzE1Mi4zMiAwIDI4Mi4xMTIgMTA4LjQxNiAzMjMuMzkyIDI2MS4xMkM5NDEuODg4IDQxMy40NCAxMDI0IDUxOS4wNCAxMDI0IDY0MC4xOTJ6IG0tMjU5LjItMjA1LjMxMmMtMjQuNDQ4LTEyOS4wMjQtMTI4Ljg5Ni0yMjIuNzItMjUyLjgtMjIyLjcyLTk3LjI4IDAtMTgzLjA0IDU3LjM0NC0yMjQuNjQgMTQ3LjQ1NmwtOS4yOCAyMC4yMjQtMjAuOTI4IDIuOTQ0Yy0xMDMuMzYgMTQuNC0xNzguMzY4IDEwNC4zMi0xNzguMzY4IDIxNC43MiAwIDExNy45NTIgODguODMyIDIxNC40IDE5Ni45MjggMjE0LjRoNTEyYzg4LjMyIDAgMTU3LjUwNC03NS4xMzYgMTU3LjUwNC0xNzEuNzEyIDAtODguMDY0LTY1LjkyLTE2NC45MjgtMTQ0Ljk2LTE3MS43NzZsLTI5LjUwNC0yLjU2LTUuODg4LTMwLjk3NnoiIGZpbGw9IiNmZmZmZmYiIHAtaWQ9IjM0MjIiIGRhdGEtc3BtLWFuY2hvci1pZD0iYTMxM3guNzc4MTA2OS4wLmkwIiBjbGFzcz0iIj48L3BhdGg+PC9zdmc+&style=for-the-badge" height="22">][crates-url]
<img alt="license" src="https://img.shields.io/badge/License-Apache%202.0/MIT-blue.svg?style=for-the-badge&fontColor=white&logoColor=f5c076&logo=data:image/svg+xml;base64,PCFET0NUWVBFIHN2ZyBQVUJMSUMgIi0vL1czQy8vRFREIFNWRyAxLjEvL0VOIiAiaHR0cDovL3d3dy53My5vcmcvR3JhcGhpY3MvU1ZHLzEuMS9EVEQvc3ZnMTEuZHRkIj4KDTwhLS0gVXBsb2FkZWQgdG86IFNWRyBSZXBvLCB3d3cuc3ZncmVwby5jb20sIFRyYW5zZm9ybWVkIGJ5OiBTVkcgUmVwbyBNaXhlciBUb29scyAtLT4KPHN2ZyBmaWxsPSIjZmZmZmZmIiBoZWlnaHQ9IjgwMHB4IiB3aWR0aD0iODAwcHgiIHZlcnNpb249IjEuMSIgaWQ9IkNhcGFfMSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIiB4bWxuczp4bGluaz0iaHR0cDovL3d3dy53My5vcmcvMTk5OS94bGluayIgdmlld0JveD0iMCAwIDI3Ni43MTUgMjc2LjcxNSIgeG1sOnNwYWNlPSJwcmVzZXJ2ZSIgc3Ryb2tlPSIjZmZmZmZmIj4KDTxnIGlkPSJTVkdSZXBvX2JnQ2FycmllciIgc3Ryb2tlLXdpZHRoPSIwIi8+Cg08ZyBpZD0iU1ZHUmVwb190cmFjZXJDYXJyaWVyIiBzdHJva2UtbGluZWNhcD0icm91bmQiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz4KDTxnIGlkPSJTVkdSZXBvX2ljb25DYXJyaWVyIj4gPGc+IDxwYXRoIGQ9Ik0xMzguMzU3LDBDNjIuMDY2LDAsMCw2Mi4wNjYsMCwxMzguMzU3czYyLjA2NiwxMzguMzU3LDEzOC4zNTcsMTM4LjM1N3MxMzguMzU3LTYyLjA2NiwxMzguMzU3LTEzOC4zNTcgUzIxNC42NDgsMCwxMzguMzU3LDB6IE0xMzguMzU3LDI1OC43MTVDNzEuOTkyLDI1OC43MTUsMTgsMjA0LjcyMywxOCwxMzguMzU3UzcxLjk5MiwxOCwxMzguMzU3LDE4IHMxMjAuMzU3LDUzLjk5MiwxMjAuMzU3LDEyMC4zNTdTMjA0LjcyMywyNTguNzE1LDEzOC4zNTcsMjU4LjcxNXoiLz4gPHBhdGggZD0iTTE5NC43OTgsMTYwLjkwM2MtNC4xODgtMi42NzctOS43NTMtMS40NTQtMTIuNDMyLDIuNzMyYy04LjY5NCwxMy41OTMtMjMuNTAzLDIxLjcwOC0zOS42MTQsMjEuNzA4IGMtMjUuOTA4LDAtNDYuOTg1LTIxLjA3OC00Ni45ODUtNDYuOTg2czIxLjA3Ny00Ni45ODYsNDYuOTg1LTQ2Ljk4NmMxNS42MzMsMCwzMC4yLDcuNzQ3LDM4Ljk2OCwyMC43MjMgYzIuNzgyLDQuMTE3LDguMzc1LDUuMjAxLDEyLjQ5NiwyLjQxOGM0LjExOC0yLjc4Miw1LjIwMS04LjM3NywyLjQxOC0xMi40OTZjLTEyLjExOC0xNy45MzctMzIuMjYyLTI4LjY0NS01My44ODItMjguNjQ1IGMtMzUuODMzLDAtNjQuOTg1LDI5LjE1Mi02NC45ODUsNjQuOTg2czI5LjE1Miw2NC45ODYsNjQuOTg1LDY0Ljk4NmMyMi4yODEsMCw0Mi43NTktMTEuMjE4LDU0Ljc3OC0zMC4wMDkgQzIwMC4yMDgsMTY5LjE0NywxOTguOTg1LDE2My41ODIsMTk0Ljc5OCwxNjAuOTAzeiIvPiA8L2c+IDwvZz4KDTwvc3ZnPg==" height="22">

</div>

## Installation

```toml
[dependencies]
soundevents-dataset = "0.4"
```

By default this pulls in the [`rated`](#rated--audioset-rated-label-set-527-entries) module — the 527-class label set used by released AudioSet/YAMNet/VGGish models. To use the [`ontology`](#ontology--full-audioset-taxonomy-632-entries) view instead (or in addition), pick the features explicitly:

```toml
# Just the full AudioSet ontology, no rated set.
soundevents-dataset = { version = "0.4", default-features = false, features = ["std", "ontology"] }

# Both views.
soundevents-dataset = { version = "0.4", features = ["ontology"] }
```

## Two views, two modules

| Module | Source | Entries | Use when… |
| --- | --- | --- | --- |
| [`rated`](#rated--audioset-rated-label-set-527-entries) | `class_labels_indices.csv` | **527** | You're working with model outputs / multi-hot label tensors. Each entry carries its `index` so the position in a 527-vector resolves to a name in `O(1)`. |
| [`ontology`](#ontology--full-audioset-taxonomy-632-entries) | `ontology.json` | **632** | You need the full taxonomy, including abstract container nodes (`"Human voice"`, `"Music"`, …) and the 105 entries that aren't in the released rated set. |

The two are independent: each lives in its own module, has its own `&'static` consts, its own perfect-hash map, and its own type (`SoundEvent` vs `RatedSoundEvent`). Enable only what you need to keep the binary small. They do share one thing — the permanent ids below, which span the full ontology, so a class in both views carries the same id in each.

## Permanent ids

Every entry carries a `SoundEventId` — a permanent, stable `u16` handle you can store now and resolve back later:

```rust
# #[cfg(feature = "rated")]
# fn main() {
use soundevents_dataset::{RatedSoundEvent, SoundEventId};

let event = RatedSoundEvent::from_key("Speech")[0];
let stored: u16 = event.id().get();   // → a database column, a wire field

let recovered = RatedSoundEvent::from_id(SoundEventId::new(stored)).expect("assigned id");
assert_eq!(recovered.mid(), event.mid());

// 0 is never assigned, so a zeroed column fails to resolve.
assert!(RatedSoundEvent::from_id(SoundEventId::new(0)).is_none());
# }
# #[cfg(not(feature = "rated"))]
# fn main() {}
```

`id` and `from_id` are a bijection onto the ids a view carries, so a downstream store never has to mint an identifier of its own — it keeps two bytes and looks the entry back up. `from_id` is total: an id the view does not carry answers `None`, never a neighbouring class.

The ids obey one discipline, and it is what makes them worth storing:

- An id is assigned once and **never changes**. Correcting an entry's display name, description, citation, children or restrictions keeps its id.
- A **dropped class's id is never reused** — `from_id` answers `None` for it forever after.
- A **new class mints a fresh id**, above every id ever assigned.

Ids start at 1, so a zeroed column never resolves.

Three things an id is deliberately *not*:

| | |
| --- | --- |
| the AudioSet **mid** (`mid()`) | Upstream's identifier, kept as provenance. It is the ledger's join key, but it is a string — wider on the wire, wider as a column, and unordered. |
| the **code** (`encode()`) | A 32-bit hash *derived* from the mid, so it cannot outlive a change to one. An id is assigned, so it can. |
| the **model output index** (`index()`, `rated` only) | A position in a released model's output vector. It moves whenever upstream retrains. |

The assignment lives in [`assets/sound_ids.csv`](./assets/sound_ids.csv) — the ledger the codegen reads, extends, and rewrites, and the reviewable form of what the generated tables ship. `tests/ids.rs` pins the complete assignment and CI re-runs the codegen, so a regeneration that renumbers anything fails loudly.

### `rated` — AudioSet rated label set (527 entries)

`RatedSoundEvent` exposes the same metadata accessors as `SoundEvent` (`id`, `mid`, `name`, `description`, `aliases`, `citation_uri`, `children`, `restrictions`) plus a rated-only [`index()`](https://docs.rs/soundevents-dataset) — the integer 0..527 used as the position in released AudioSet models' output vectors. Walking `children()` stays inside the rated namespace: any ontology child that is *not* in the rated set is dropped, so the hierarchy remains self-consistent.

### Case-insensitive, separator-distinct lookup

`from_key` is keyed by [`UncasedStr`](https://docs.rs/uncased), so any case form of a mid or alias resolves to the same entry without us having to enumerate every possibility:

Separator styles are still indexed independently (`"man speaking"` ≠ `"man_speaking"` ≠ `"man-speaking"` ≠ `"manSpeaking"`), so you only pay for the four shapes the codegen actually emits — every case variant of each shape collapses into one phf bucket.

## Features

| Feature | Default | What you get |
| --- | :-: | --- |
| `std` | ✓ | Standard library + `std`-dependent error reporting via `thiserror`. Disable for `no_std`. |
| `rated` | ✓ | The `rated` module (527 entries, ~1900 phf keys). |
| `ontology` | | The `ontology` module (632 entries, ~2400 phf keys). |
| `alloc` | | Opt-in `alloc` support for `no_std` targets with an allocator. |
| `serde` | | Derives `Serialize` for `SoundEvent`, `RatedSoundEvent`, and `Restriction`. |

The crate is `#![no_std]`-compatible (`default-features = false`). The entire dataset lives in `&'static` memory: no allocations, no startup cost, and the perfect-hash map is generated by [`phf_codegen`](https://docs.rs/phf_codegen) at codegen time so the dataset crate's compile graph contains no proc-macros from `phf`.

## Regenerating the dataset

`src/ontology/generated.rs` and `src/rated/generated.rs` are checked in and produced from `assets/ontology.json` and `assets/class_labels_indices.csv` by an `xtask` binary. After updating either source file or the codegen logic, regenerate both:

```sh
cargo xtask codegen && cargo fmt --all
```

The second step is not optional: the generator emits `prettyplease` output, which is fixed at four-space indent, while `rustfmt.toml` sets `tab_spaces = 2`. Skipping `cargo fmt` leaves a ~30,000-line whitespace diff against the checked-in tables.

Each entry's `code` is a 32-bit hash of its mid, and codegen panics rather than emit a table in which two mids hash to the same code.

Codegen also resolves each entry's permanent id against `assets/sound_ids.csv`, minting fresh ids above the high-water mark for mids the ledger has never seen and rewriting it. **The ledger is an input, not an output** — ids already assigned never move, and a class dropped from the ontology keeps its row, retired, so its number is never handed out again. Every row is load-bearing: a retired one is the only record that its id was spent, so the file only ever grows and is never regenerated from scratch.

A missing or emptied ledger is an unconditional hard error, and an emptied one is refused even by the genesis path below: a file with no rows is a *lost* ledger wearing a header, not a new one, and accepting it would remint the whole dataset from 1 and lose every retired id. Restore it from version control.

One escape hatch, off by default because the safe behavior is to stop and ask:

| Flag | When codegen refuses without it |
| --- | --- |
| `--allow-retire-and-mint` | One run both retires a mid and mints another. That is what upstream *re-midding* a class it kept would look like from here, and minting would break every id already stored for it. Fix it in place — edit the mid in the ledger, keeping its id — or pass this to say the two events are unrelated. |

Creating the ledger is a separate one-shot command, `cargo xtask bootstrap-ledger`, and never a flag on a normal run — on the normal path a *lost* ledger is indistinguishable from a never-created one, and minting would restart at 1 in the current ontology order, handing numbers already in databases to different classes with every other guard silent. It refuses unless the dataset has demonstrably never shipped an id: the ledger must be **absent** rather than empty, and neither committed `generated.rs` may already carry ids.

CI's `codegen-up-to-date` job re-runs the xtask and fails if either generated file or `sound_ids.csv` changes or goes untracked — guaranteeing no drift between `assets/` and the committed source, and catching a class whose id was minted in CI rather than committed. `tests/ids.rs` pins the ledger separately, retired rows included, because a lost tombstone is invisible to everything else: the tables, the generated files and the codegen diff all agree with the shortened ledger.

#### License

`soundevents-dataset` is under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.
Bundled AudioSet metadata attribution and upstream license details are
documented in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Copyright (c) 2026 FinDIT studio authors.

[Github-url]: https://github.com/findit-studio/soundevents
[CI-url]: https://github.com/findit-studio/soundevents/actions/workflows/ci.yml
[doc-url]: https://docs.rs/soundevents-dataset
[crates-url]: https://crates.io/crates/soundevents-dataset
[codecov-url]: https://app.codecov.io/gh/findit-studio/soundevents/
