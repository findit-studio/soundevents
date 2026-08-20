# Changelog

All notable changes to this workspace will be documented in this file.

## soundevents-dataset 0.3.1 - 2026-08-20

### `soundevents-dataset`

- The `rated` module's doc no longer claims blacklisted classes are excluded: 12 of the 527 released classes carry the ontology's blacklist restriction and are kept — upstream published them, and a model still scores their `index` slot. A test now pins the count so upstream drift trips it.
- Added `is_blacklisted()`, a `const` convenience predicate beside `restrictions()`, on both `SoundEvent` and `RatedSoundEvent`.
- `rust-version` raised `1.59.0` → `1.85`: the manifest now states the floor `phf` 0.14 already imposed, resolving the decision the 0.4.0 section below left open (0.3.0 shipped with the stale floor still declared).

## 0.4.0 - 2026-07-31

### `soundevents-dataset`

- Rebuilt the generated lookup tables against `phf` 0.14. `phf` 0.14 changes how a key is hashed, so the perfect-hash parameters in the generated static move; the key set, every sound-event code, and every `from_key` result are unchanged. `phf` does not appear in this crate's public API.
- Raises the toolchain this crate actually builds on to Rust 1.85, because `phf` 0.14 is an edition-2024 crate declaring `rust-version = "1.85"`. The manifest still declares `rust-version = "1.59.0"`; that floor is no longer reachable and needs a decision before this version is released.
- Added test coverage that resolves every id and every alias in both tables through `from_key`, in each ASCII case form, rather than the previous handful of sampled keys.
- Breaking changes:
  - Sound-event codes are now `i64` instead of `u64`. `SoundEvent::encode`, `RatedSoundEvent::encode`, `from_code`, `UnknownSoundEventCode::code`, and `UnknownRatedSoundEventCode::code` all change type, and the `TryFrom<u64>` conversions become `TryFrom<i64>`.
  - Every code value changed. A code is now a 32-bit hash of the entry's id — the AudioSet MID — instead of a 64-bit hash of its display name, so it no longer moves when upstream relabels an entry, and it lands in `0..=u32::MAX`: never negative, and well inside the signed 64-bit key range that databases accept without a rebind scheme. Persisted codes must be recomputed. An id present in both views still carries the same code in each.
  - Code generation now refuses to emit a table containing a code collision, so uniqueness is checked at build time rather than assumed.

### `soundevents`

- Fixed: the crate did not build from a clean dependency resolution. `ort = "2.0.0-rc.12"` is a caret range, and a caret on a pre-release admits later pre-releases of the same version, so a fresh resolve selected `ort` 2.0.0-rc.13 — which marked `GraphOptimizationLevel` `#[non_exhaustive]`, breaking the exhaustive `match` behind the `serde` feature with `E0004`.
- With the `serde` feature, a graph optimization level this crate does not recognise now serializes as `disable` rather than failing to compile. `ort` 2.0.0-rc.13 adds no new levels, so no currently reachable value changes representation.
- Enabling a combination of execution-provider features for which no prebuilt ONNX Runtime is published now fails at link time. `ort` 2.0.0-rc.13 refuses to substitute a distribution that lacks the requested providers, where rc.12 silently linked the provider-less build instead — so such a combination never delivered the providers it named, it only failed quietly. Enable just the providers published for your target, or add `ort`'s `lax-feature-matching` feature to accept a substitute.
- Breaking changes:
  - Re-released against `soundevents-dataset` 0.3, so the `RatedSoundEvent` reachable through `ScoredEvent::event` carries the new `i64` code type.
  - Requires `ort` 2.0.0-rc.13. `ort` types are part of this crate's public API (`Options::optimization_level`, `ClassifierError::Ort`), so a downstream crate that also depends on `ort` must move to rc.13 alongside this one.

### `xtask`

- Moved code generation to `phf_codegen`/`phf_shared` 0.14, `syn` 3, and `prettyplease` 0.3. `phf_codegen` writes the static that `phf` reads, and `prettyplease` prints `syn`'s AST, so each pair has to move together: a generator and runtime on different `phf` versions would emit a table whose keys the runtime hashes differently, which no test of the codegen alone would catch.
- `syn` 3 removes the second copy of `syn` from the build — `serde_derive` and `thiserror-impl` already required it, so `xtask` was compiling `syn` twice.

## 0.3.0 - 2026-04-21

### `soundevents`

- Add serde support for `Options` and `ChunkingOptions`.
- Breaking changes:
  - `Options::new` no longer takes a model path; construct options separately from model loading.
  - Chunked classification APIs now take `&ChunkingOptions` instead of `ChunkingOptions`.
  - `ChunkingOptions` is no longer `Copy`; pass it by reference or clone it explicitly where needed.

## 0.2.0 - 2026-04-08

### `soundevents`

- Added `predict_raw_scores_batch_flat` and `predict_raw_scores_batch_into` for lower-allocation batched raw-score access.
- Expanded batched inference coverage with regression tests that verify flat and buffer-reuse paths against sequential inference.
- Removed redundant input validation in `classify_batch` while preserving the existing error behavior for invalid batches.
- Tightened crate metadata and docs.rs configuration so feature-gated APIs, including `Classifier::tiny`, render correctly on published docs.
- Added packaged third-party notices for bundled CED model artifacts.

### `soundevents-dataset`

- Packaged the dual-license texts with the published crate and aligned crate metadata for docs.rs and crates.io discovery.
- Kept the crate on its Rust 1.59 / edition 2021 compatibility track while removing the in-source `deny(warnings)` footgun.
- Added packaged third-party notices for bundled AudioSet ontology and label metadata.

### Workspace

- Included license files in published package contents for both crates.
- Upgraded README examples from ignored snippets to compile-checked doctests across the workspace.

## 0.1.0 - 2026-04-08

### `soundevents`

- Initial public release of the ONNX Runtime wrapper for CED AudioSet classifiers.
- Added file, memory, and bundled-model loading paths plus configurable graph optimization.
- Added ranked top-k helpers, raw-score accessors, and chunked inference with mean/max aggregation.
- Added equal-length batch APIs for clip inference and chunked window batching for higher-throughput services.

### `soundevents-dataset`

- Initial public release of the typed AudioSet dataset companion crate.
- Included both the 527-class rated label set and the full 632-entry ontology as `&'static` generated data.
- Kept the crate `no_std`-friendly, allocation-free at runtime, and compatible with Rust 1.59.

### `xtask`

- Added code generation for the rated label set and ontology modules from upstream AudioSet source data.
