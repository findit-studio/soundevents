//! Code generator for `soundevents-dataset`.
//!
//! Reads `assets/ontology.json` plus `assets/class_labels_indices.csv` and
//! emits `src/ontology/generated.rs` and `src/rated/generated.rs`.
//!
//! Each entry's permanent `SoundEventId` comes from
//! `soundevents-dataset/assets/sound_ids.csv`, the id ledger — see
//! [`Ledger`] for the discipline it enforces. The ledger is read *and
//! rewritten* by this tool; it and both `generated.rs` files are
//! committed, and CI fails on drift in any of them.
//!
//! Run with: `cargo xtask codegen && cargo fmt --all`.

use std::{
  collections::{BTreeMap, BTreeSet, HashMap, HashSet},
  io::Write as _,
  path::{Path, PathBuf},
};

use heck::{
  ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToShoutySnekCase,
  ToSnakeCase, ToTitleCase as _, ToTrainCase as _, ToUpperCamelCase as _,
};
use indexmap::{IndexMap, IndexSet};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use syn::parse::{Parse, Parser};
use uncased::UncasedStr;
use xxhash_rust::const_xxh32::xxh32;

/// Seed for the entry-code hash. Not a tuning knob: changing it rotates every
/// code in both tables at once, so any stored code becomes unresolvable.
const CODE_SEED: u32 = 0;

/// A tag for the audioset (mirrors `ontology.json` schema).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct RawSoundEvent {
  id: String,
  name: String,
  description: String,
  citation_uri: String,
  positive_examples: Vec<String>,
  child_ids: Vec<String>,
  restrictions: Vec<String>,
}

/// One row of `class_labels_indices.csv`.
#[derive(Debug, Clone, Deserialize)]
struct CsvRow {
  index: usize,
  mid: String,
  #[allow(dead_code)]
  display_name: String,
}

/// One row of `assets/sound_ids.csv`, the permanent-id ledger.
#[derive(Debug, Deserialize, Serialize)]
struct LedgerRow {
  id: u16,
  mid: String,
}

/// Prologue rewritten verbatim on top of `assets/sound_ids.csv` every
/// codegen run. The ledger carries its own law so a curator editing it
/// by hand reads the discipline before touching a number.
const LEDGER_HEADER: &str = "\
# soundevents-dataset — the permanent sound-event-id ledger.
#
# THIS FILE IS THE AUTHORITY FOR `SoundEventId`. `cargo xtask codegen`
# reads it, mints ids for any new `ontology.json` entry, rewrites it, and
# emits the ids into `src/ontology/generated.rs` and
# `src/rated/generated.rs`, where they are part of the crate's PUBLIC API.
# Downstream stores the id and looks the entry back up; a renumbering
# silently repoints stored data at the wrong sound event.
#
# The id discipline — these ids are PERMANENT:
#
#   * An id is assigned once and NEVER changes. Correcting an entry's
#     display name, description, citation, children or restrictions
#     KEEPS its id.
#   * A dropped entry's id is NEVER reused. Its row stays here, retired,
#     so the mint can never hand the number out again.
#   * A new entry mints a fresh id: the high-water mark plus one. Ids
#     start at 1 — 0 is never assigned, so a zeroed id is always
#     detectably invalid.
#
# EVERY ROW HERE IS LOAD-BEARING, retired ones included. A retired row
# is the only record that its number was ever handed out; delete it and
# the next mint can hand the same number to a different sound event.
# Rows are only ever added or edited in place — never removed, never
# renumbered, and the file is never regenerated from scratch.
# `tests/ids.rs` pins every row, retired ones included, and CI re-runs
# codegen and fails on any drift in this file.
#
# `mid` is the AudioSet machine id (`/m/09x0r`, `/t/dd00012`) and the
# join key the generator matches `ontology.json` entries on. Upstream
# treats it as the entry's stable identity and relabels the display name
# freely, so a rename never reaches this file: it is only ever a
# correction to `ontology.json`, and the id stays put.
#
# A mid, in turn, is not supposed to change — so a run that both retires
# a mid and mints a new one is either a genuine class swap in an upstream
# revision, or upstream RE-MIDDING a class it kept. The second would
# orphan every id already stored for it, and codegen cannot tell the two
# apart. It refuses such a run until you either edit the mid in place
# here (keeping its id) or pass --allow-retire-and-mint to confirm the
# events are unrelated.
#
# The ledger spans the FULL ontology, and the `rated` module is a subset
# of it keyed on the same mids — so a class present in both views carries
# one and the same id in each. `rated`'s model output index is a separate,
# retrain-scoped number and is NOT this id.
";

/// The permanent-id ledger: the id half of the `SoundEventId` bijection,
/// kept beside `ontology.json` rather than inside it so that upstream
/// file stays verbatim-replaceable on an AudioSet refresh.
///
/// The ledger holds *every* mid ever seen, live and retired. Retirement
/// is what burns an id: a row with no matching ontology entry is never
/// matched and still counts toward [`Ledger::high_water`], so its number
/// can never be minted a second time.
struct Ledger {
  /// Mid → id for every row ever minted, live and retired.
  by_mid: BTreeMap<String, u16>,
  /// What the file held on entry. Re-checked before writing so a future
  /// edit to this type cannot silently move an already-assigned id.
  loaded: BTreeMap<String, u16>,
  /// Highest id ever minted, retired rows included.
  high_water: u16,
  /// Mids matched against an ontology entry this run; the complement is
  /// retired.
  live: BTreeSet<String>,
  /// Ids minted this run, for the codegen log.
  minted: Vec<(u16, String)>,
  /// The exact bytes [`Self::load`] read, or `None` for a ledger that was
  /// not read from a file. [`Self::write`] refuses to install over
  /// anything else, so a run whose snapshot went stale cannot delete the
  /// ids another run assigned in the meantime.
  loaded_bytes: Option<Vec<u8>>,
  /// Where these rows came from, for error messages.
  source: String,
}

impl Ledger {
  /// Validate a set of ledger rows and take ownership of them.
  ///
  /// Panics on a malformed ledger — a duplicate id, a duplicate mid, or
  /// the reserved id 0 — rather than generating a table whose ids are
  /// not a bijection. Split from [`Self::load`] so the discipline can be
  /// exercised in unit tests without touching the filesystem.
  fn from_rows(rows: Vec<LedgerRow>, source: &str) -> Self {
    let mut by_mid = BTreeMap::<String, u16>::new();
    let mut by_id = BTreeMap::<u16, String>::new();

    for LedgerRow { id, mid } in rows {
      assert!(
        id != 0,
        "{source}: id 0 is reserved as \"never assigned\" (row {mid:?})",
      );
      if let Some(other) = by_id.insert(id, mid.clone()) {
        panic!(
          "{source}: id {id} is assigned twice, to {other:?} and {mid:?}; \
           ids are permanent and unique — resolve by hand, never by renumbering",
        );
      }
      if let Some(other) = by_mid.insert(mid.clone(), id) {
        panic!(
          "{source}: {mid:?} appears twice, as id {other} and id {id}; \
           a mid is the ledger's join key and must be unique",
        );
      }
    }

    let high_water = by_id.keys().next_back().copied().unwrap_or(0);
    Self {
      loaded: by_mid.clone(),
      by_mid,
      high_water,
      live: BTreeSet::new(),
      minted: Vec::new(),
      loaded_bytes: None,
      source: source.to_string(),
    }
  }

  /// Read the committed ledger.
  ///
  /// A missing file is a hard error unless this is a genesis run. That
  /// asymmetry is deliberate: the ledger is the only record that a
  /// retired id was ever handed out, so silently regenerating it from
  /// `ontology.json` would renumber the dataset and remint every retired
  /// number. Creating one is an explicit, once-ever act — the
  /// `bootstrap-ledger` command, which proves it is genesis before it
  /// gets here — and never a fallback on the normal path.
  ///
  /// An EMPTY ledger is refused unconditionally, genesis included: a
  /// file that exists but holds no rows is a LOST ledger wearing a
  /// header, not a new one, and genesis requires absence.
  fn load(path: &Path, genesis: Genesis) -> Self {
    let source = path.display().to_string();

    if !path.exists() {
      assert!(
        genesis == Genesis::Allowed,
        "{source}: the permanent-id ledger is missing. It is the only \
         record of which ids have been handed out, including retired \
         ones — regenerating it from ontology.json would renumber the \
         dataset and remint retired ids. Restore the file from version \
         control. `cargo xtask bootstrap-ledger` creates one only for a \
         dataset that has never shipped an id.",
      );
      return Self::from_rows(Vec::new(), &source);
    }

    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("open {source}: {e}"));
    let rows = Self::rows_from_csv(&bytes, &source);

    // An emptied ledger is a lost ledger wearing a header, and NOTHING
    // excuses it — genesis least of all, because genesis is exactly the
    // mode that would then remint the whole dataset from 1 in the
    // current ontology order, handing numbers already in databases to
    // different classes. It retires nothing, so the retire-and-mint gate
    // stays quiet; the write-time seal has only this empty baseline to
    // compare against; and the install succeeds. Genesis requires the
    // file to be ABSENT. A file that exists must hold rows.
    assert!(
      !rows.is_empty(),
      "{source}: the permanent-id ledger has no rows. An empty ledger \
       remints every id from 1 and loses every retired one, so this is a \
       lost ledger rather than a new one. Restore the file from version \
       control; no flag and no command overrides this.",
    );

    let mut ledger = Self::from_rows(rows, &source);
    ledger.loaded_bytes = Some(bytes);
    ledger
  }

  /// Parse ledger rows out of raw CSV bytes.
  ///
  /// Shared by [`Self::load`] and [`Self::write`] so the ledger is read
  /// back exactly the way it is read in: a dialect that drifted between
  /// the two would let [`Self::write`] bless a file that the next
  /// [`Self::load`] reads differently.
  fn rows_from_csv(bytes: &[u8], source: &str) -> Vec<LedgerRow> {
    let mut rdr = csv::ReaderBuilder::new()
      .has_headers(true)
      .comment(Some(b'#'))
      .from_reader(bytes);
    rdr
      .deserialize::<LedgerRow>()
      .map(|row| row.unwrap_or_else(|e| panic!("parse {source}: {e}")))
      .collect()
  }

  /// Where a replacement ledger is staged before it is installed: a
  /// sibling of the ledger itself, so installing it is a same-directory
  /// rename rather than a copy across filesystems. The pid keeps two
  /// concurrent runs off each other's staging file.
  fn staging_path(path: &Path) -> PathBuf {
    let name = path
      .file_name()
      .unwrap_or_else(|| panic!("{}: ledger path has no file name", path.display()));
    let mut staged = name.to_os_string();
    staged.push(format!(".tmp.{}", std::process::id()));
    path.with_file_name(staged)
  }

  /// The id for `mid`: its existing one, or a freshly minted one above
  /// the high-water mark. Marks the mid live either way.
  fn id_for(&mut self, mid: &str) -> u16 {
    self.live.insert(mid.to_string());
    if let Some(&id) = self.by_mid.get(mid) {
      return id;
    }
    let id = self
      .high_water
      .checked_add(1)
      .expect("permanent sound-event ids exhausted u16; widen SoundEventId's repr");
    self.high_water = id;
    self.by_mid.insert(mid.to_string(), id);
    self.minted.push((id, mid.to_string()));
    id
  }

  /// Ledger rows with no matching ontology entry this run. Their ids
  /// stay burned; the list is reported so a retirement is never silent.
  fn retired(&self) -> Vec<(u16, &str)> {
    let mut out: Vec<(u16, &str)> = self
      .by_mid
      .iter()
      .filter(|(mid, _)| !self.live.contains(*mid))
      .map(|(mid, &id)| (id, mid.as_str()))
      .collect();
    out.sort_unstable();
    out
  }

  /// Refuse a run that both retires a mid and mints a new one, unless
  /// the operator has said the two are unrelated.
  ///
  /// AudioSet treats a mid as a class's stable identity, which is why
  /// this generator joins on it — but nothing enforces that upstream. If
  /// a revision ever RE-MIDS a class it kept, it reaches the generator as
  /// a retirement plus a mint, and minting would silently break every id
  /// already stored for that class. That is indistinguishable from the
  /// benign case — one class genuinely dropped, another genuinely added —
  /// so codegen stops and asks rather than guessing.
  ///
  /// A run that only mints (new classes) or only retires (dropped
  /// classes) is unambiguous and passes untouched.
  fn assert_rename_resolved(&self, allow_retire_and_mint: bool) {
    let retired = self.retired();
    if allow_retire_and_mint || self.minted.is_empty() || retired.is_empty() {
      return;
    }

    let minted = self
      .minted
      .iter()
      .map(|(id, mid)| format!("\n    would mint id {id} for {mid:?}"))
      .collect::<String>();
    let retired = retired
      .iter()
      .map(|(id, mid)| format!("\n    would retire id {id}, held by {mid:?}"))
      .collect::<String>();
    panic!(
      "{}: this run both retires and mints, which is what an upstream \
       RE-MIDDING looks like from here — and minting would break every id \
       already stored for the re-midded class.\
       \n{retired}{minted}\
       \n\n  If a retirement above is really the same class under a new \
       mid, that is a CORRECTION: edit the mid in place in the ledger, \
       keeping its id, and re-run. The pairing disappears and codegen \
       proceeds.\
       \n  If they are genuinely unrelated — a class left and a different \
       one arrived — re-run with --allow-retire-and-mint to say so.",
      self.source,
    );
  }

  /// Rewrite the ledger, sorted by id. Re-asserts the seal first: every
  /// id present on entry must still map to the same mid.
  ///
  /// The replacement is installed atomically, and that is load-bearing
  /// rather than tidy. Writing over the ledger in place truncates it
  /// first, so an I/O error, a kill, or a power loss partway through
  /// leaves a PREFIX of the new rows — and a prefix is not detectably
  /// broken. [`Self::to_csv`] emits rows in id order, so one ending on a
  /// row boundary parses cleanly, still looks dense, and silently drops
  /// the HIGHEST ids: [`Self::load`] accepts it (it refuses only a
  /// missing or empty ledger), [`Self::high_water`] falls, the dropped
  /// mids look new, and [`Self::assert_rename_resolved`] sees pure
  /// minting — so spent ids are handed back out to other classes with
  /// every gate silent. The seal above cannot see it either: it compares
  /// against what this run loaded, which is the truncated file.
  ///
  /// So the whole file is staged beside the ledger, fsynced, checked to
  /// read back as exactly this ledger, and only then renamed over it. A
  /// reader sees the old ledger or the new one, never a prefix, and any
  /// failure before the rename leaves the original untouched.
  fn write(&self, path: &Path) {
    for (mid, &id) in &self.loaded {
      let now = self.by_mid.get(mid).copied();
      assert_eq!(
        now,
        Some(id),
        "{}: {mid:?} held id {id} on entry and {now:?} on exit; sound-event \
         ids are PERMANENT — a correction keeps its id and a retired id is \
         never reused",
        path.display(),
      );
    }

    let body = self.to_csv();

    // Read the bytes back before they become the ledger. The file is the
    // only record there is — nothing can reconstruct it — so a serializer
    // that dropped or merged a row must fail here rather than on disk.
    let staged_rows = Self::rows_from_csv(body.as_bytes(), &self.source);
    let staged_by_mid: BTreeMap<&str, u16> = staged_rows
      .iter()
      .map(|row| (row.mid.as_str(), row.id))
      .collect();
    let expected: BTreeMap<&str, u16> = self
      .by_mid
      .iter()
      .map(|(mid, &id)| (mid.as_str(), id))
      .collect();
    assert_eq!(
      staged_rows.len(),
      expected.len(),
      "{}: the serialized ledger holds {} rows for {} mids; refusing to install it",
      path.display(),
      staged_rows.len(),
      expected.len(),
    );
    assert_eq!(
      staged_by_mid,
      expected,
      "{}: the serialized ledger does not read back as the ledger it came from; \
       refusing to install it",
      path.display(),
    );

    let staged = Self::staging_path(path);
    if let Err(e) = Self::stage(&staged, body.as_bytes()) {
      // Leave nothing beside the ledger: an untracked file in `assets/`
      // is litter the next run would have to reason about.
      std::fs::remove_file(&staged).ok();
      panic!(
        "write {}: staging as {}: {e}",
        path.display(),
        staged.display()
      );
    }
    // Compare-and-swap, as late as it can be placed: everything slow —
    // serializing, staging, syncing — is already done, so the window
    // between the check and the rename is a read and a rename and
    // nothing else. It is a BACKSTOP, not the protocol. The transaction
    // lock in `codegen` is what makes this run the single writer; this
    // catches a writer that never took it (a hand edit, an older
    // binary), and no check-then-act on a filesystem can do better than
    // narrow the window, because `std` has no atomic compare-and-rename.
    Self::assert_unchanged_since_load(self.loaded_bytes.as_deref(), path, Some(&staged));

    // Atomic on POSIX; MOVEFILE_REPLACE_EXISTING on Windows.
    if let Err(e) = std::fs::rename(&staged, path) {
      std::fs::remove_file(&staged).ok();
      panic!(
        "write {}: installing {}: {e}",
        path.display(),
        staged.display()
      );
    }
    // The rename is atomic but not yet durable: until the directory entry
    // is persisted, a power loss can put the OLD ledger back after this
    // returned success — silently dropping this run's mints while the
    // generated tables that used them go on to be written. That is a
    // failed install, so it is an error rather than a shrug.
    if let Err(e) = Self::sync_install(path) {
      panic!(
        "write {}: installed, but the install could not be made durable ({e}); \
         a power loss could restore the previous ledger and drop the ids \
         minted by this run. Re-run codegen.",
        path.display(),
      );
    }
  }

  /// Refuse to install unless the ledger on disk is still byte-identical
  /// to the snapshot this run loaded.
  ///
  /// Two runs that read one ledger mint from one high-water mark, so
  /// whichever renames last would install a ledger that never saw the
  /// other one's assignment — deleting a successfully minted id and
  /// freeing that number for a different class, with both runs reporting
  /// success. `staged`, when given, is removed before panicking so a
  /// refusal leaves nothing behind.
  fn assert_unchanged_since_load(loaded: Option<&[u8]>, path: &Path, staged: Option<&Path>) {
    let refuse = |reason: String| -> ! {
      if let Some(staged) = staged {
        std::fs::remove_file(staged).ok();
      }
      panic!("{}: {reason}", path.display())
    };

    match (loaded, std::fs::read(path)) {
      (Some(loaded), Ok(current)) => {
        if current != loaded {
          refuse(
            "the ledger changed on disk while this run was working. Another \
             codegen run, or an edit, has assigned ids since this one read it \
             — installing now would delete them. Re-run codegen."
              .to_string(),
          );
        }
      }
      (None, Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
      (Some(_), Err(e)) => refuse(format!(
        "the ledger this run read is gone ({e}); refusing to install over \
         whatever replaced it. Restore it from version control and re-run \
         codegen."
      )),
      (None, Ok(_)) => refuse(
        "a ledger appeared where this run found none; it may hold ids this \
         run knows nothing about, and installing would delete them. Re-run \
         codegen."
          .to_string(),
      ),
      (None, Err(e)) => refuse(format!("{e}")),
    }
  }

  /// Write the replacement out and make its CONTENTS durable, before it
  /// is reachable under the ledger name. A rename that outlived its own
  /// bytes would install an empty file.
  fn stage(staged: &Path, body: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(staged)?;
    file.write_all(body)?;
    file.sync_all()
  }

  /// Make the INSTALL durable — the directory entry the rename created,
  /// not the file contents, which [`Self::stage`] already synced.
  #[cfg(unix)]
  fn sync_install(path: &Path) -> std::io::Result<()> {
    let dir = path
      .parent()
      .filter(|parent| !parent.as_os_str().is_empty())
      .unwrap_or_else(|| Path::new("."));
    std::fs::File::open(dir)?.sync_all()
  }

  /// The same, as far as `std` can express it off unix: there is no
  /// directory handle to sync and no write-through move, so the installed
  /// file is flushed instead.
  ///
  /// The handle must be opened for WRITING even though nothing is
  /// written through it: `sync_all` is `FlushFileBuffers` on Windows, and
  /// that requires `GENERIC_WRITE`, so a read handle fails every install
  /// with a permission error after the rename has already happened.
  ///
  /// This flushes the file, not the directory entry the rename created,
  /// so the rename itself is not guaranteed durable here — that would
  /// need `MOVEFILE_WRITE_THROUGH`, which `std` does not expose. The gap
  /// is named rather than papered over. It is survivable: losing the
  /// rename restores the previous ledger, which is a consistent one, and
  /// re-running codegen re-mints the same ids in the same order.
  #[cfg(not(unix))]
  fn sync_install(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
      .write(true)
      .open(path)?
      .sync_all()
  }

  /// Serialize the whole ledger — header prologue, then every row, live
  /// and retired, sorted by id.
  ///
  /// Deterministic down to the byte: the row order is the id order and
  /// the terminator is always LF, so CI's byte comparison of a
  /// regenerated ledger holds across platforms.
  fn to_csv(&self) -> String {
    let mut rows: Vec<LedgerRow> = self
      .by_mid
      .iter()
      .map(|(mid, &id)| LedgerRow {
        id,
        mid: mid.clone(),
      })
      .collect();
    rows.sort_unstable_by_key(|r| r.id);

    // LF explicitly: CI's `codegen-up-to-date` job compares bytes, so a
    // regeneration on Windows must produce the same file as one on Linux.
    let mut wtr = csv::WriterBuilder::new()
      .terminator(csv::Terminator::Any(b'\n'))
      .from_writer(Vec::<u8>::new());
    for row in &rows {
      wtr.serialize(row).expect("serialize ledger row");
    }
    let body = wtr.into_inner().expect("flush ledger writer");
    let body = String::from_utf8(body).expect("ledger rows are utf-8");

    format!("{LEDGER_HEADER}{body}")
  }
}

/// An exclusive lock over one whole codegen transaction: reading every
/// input, minting into the ledger, installing it, and emitting the tables
/// that carry those ids.
///
/// Without it two runs can read the same ledger, mint from the same
/// high-water mark, and both succeed — the one that renames last
/// installing a ledger that never saw the other one's assignments, so
/// those ids are lost and free to be minted again for different classes.
/// [`Ledger::write`] refuses that install too, but a check-then-rename is
/// only ever a backstop: `std` has no atomic compare-and-rename, so the
/// lock, not the check, is the protocol's single-writer authority.
///
/// # Why the anchor is a committed file that nothing reads
///
/// Two constraints meet, and only a dedicated tracked file satisfies
/// both.
///
/// It must not be replaceable. A lock on an untracked `.lock` sibling
/// excludes nobody once some process unlinks and recreates it — the
/// holder is left locking an unreachable inode and the lock silently
/// stops locking. The ledger is no better an anchor: installing it
/// REPLACES it by rename, so the inode a holder locked is not the one
/// that ends up in place. `assets/codegen.lock` is committed, so its
/// removal is a visible change rather than a silent loss of exclusion.
///
/// It must not be a file the run reads. Windows file locks are
/// MANDATORY: a locked range cannot be read through a second handle, not
/// even by the process holding the lock. Anchoring on `ontology.json`
/// would therefore fail every Windows run at its first read.
///
/// It is an OS lock rather than a lock file, so the kernel drops it when
/// the process ends and a killed run leaves nothing to clear by hand.
struct LedgerLock {
  _file: std::fs::File,
}

impl LedgerLock {
  /// Take the lock, waiting if another run holds it. Waiting is right:
  /// the other run is minting ids this one must see.
  fn acquire(anchor: &Path) -> Self {
    let file = Self::open_anchor(anchor);
    file
      .lock()
      .unwrap_or_else(|e| panic!("lock {}: {e}", anchor.display()));

    Self { _file: file }
  }

  /// Take the lock only if it is free. Exists so the exclusion can be
  /// asserted without threads or sleeps.
  #[cfg(test)]
  fn try_acquire(anchor: &Path) -> Option<Self> {
    let file = Self::open_anchor(anchor);
    match file.try_lock() {
      Ok(()) => Some(Self { _file: file }),
      Err(std::fs::TryLockError::WouldBlock) => None,
      Err(std::fs::TryLockError::Error(e)) => {
        panic!("lock {}: {e}", anchor.display())
      }
    }
  }

  /// The anchor must already exist: it is committed, and creating one on
  /// demand would hand a run that lost it a private lock excluding
  /// nobody.
  fn open_anchor(anchor: &Path) -> std::fs::File {
    std::fs::File::open(anchor).unwrap_or_else(|e| {
      panic!(
        "{}: cannot take the codegen transaction lock: {e}. The anchor is \
         committed to the repository; restore it from version control \
         rather than recreating it, so concurrent runs lock the same file.",
        anchor.display()
      )
    })
  }
}

/// Every file one codegen run touches, resolved from the workspace root.
///
/// It is a type rather than five locals because the tests need the same
/// list: the transaction lock has to be anchored on a file that appears
/// nowhere in [`Self::read_and_written`], and a list hand-copied into a
/// test would silently stop covering the paths the moment codegen gained
/// one.
struct CodegenPaths {
  ontology_json: PathBuf,
  rated_csv: PathBuf,
  ledger: PathBuf,
  ontology_out: PathBuf,
  rated_out: PathBuf,
  /// The committed anchor for the transaction lock. See [`LedgerLock`]
  /// for why it is tracked and why nothing may read or write it.
  lock_anchor: PathBuf,
}

impl CodegenPaths {
  fn new(root: &Path) -> Self {
    Self {
      ontology_json: root.join("soundevents-dataset/assets/ontology.json"),
      rated_csv: root.join("soundevents-dataset/assets/class_labels_indices.csv"),
      ledger: root.join("soundevents-dataset/assets/sound_ids.csv"),
      ontology_out: root.join("soundevents-dataset/src/ontology/generated.rs"),
      rated_out: root.join("soundevents-dataset/src/rated/generated.rs"),
      lock_anchor: root.join("soundevents-dataset/assets/codegen.lock"),
    }
  }

  /// The files a run opens for reading or writing — everything but the
  /// lock anchor, which is only ever locked. Test-only: `codegen`
  /// destructures the paths it needs, and this exists so the anchor test
  /// cannot go stale against that list.
  ///
  /// Windows file locks are MANDATORY, so none of these may BE the
  /// anchor: a locked input cannot be read and a locked output cannot be
  /// written, not even by the process holding the lock. The staging file
  /// is absent on purpose — it is derived from the ledger path and does
  /// not exist before a run, so it can never be an anchor that
  /// [`LedgerLock::acquire`] would open.
  #[cfg(test)]
  fn read_and_written(&self) -> [&Path; 5] {
    [
      &self.ontology_json,
      &self.rated_csv,
      &self.ledger,
      &self.ontology_out,
      &self.rated_out,
    ]
  }
}

/// Whether this run may CREATE the permanent-id ledger.
///
/// Not an option on [`CodegenOptions`], because it is not an option: a
/// normal run may never create the ledger, whatever it is passed. Only
/// [`bootstrap_ledger`] can produce [`Genesis::Allowed`], and only after
/// proving that no id has ever been emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Genesis {
  /// The ledger must already exist and hold rows. Every normal run.
  Forbidden,
  /// The ledger is being created for the first and only time.
  Allowed,
}

/// The one escape hatch left on the ledger's discipline. It defaults to
/// off and exists because the safe behavior is to stop and ask: the
/// generator cannot tell an upstream re-midding from a class swap.
///
/// Creating the ledger is deliberately NOT here — see [`Genesis`].
#[derive(Debug, Default, Clone, Copy)]
struct CodegenOptions {
  /// Accept a run that both retires and mints as genuinely unrelated
  /// events rather than an unrecorded re-midding.
  allow_retire_and_mint: bool,
}

const USAGE: &str = "usage: cargo xtask [codegen] [--allow-retire-and-mint]\n\
                     \x20      cargo xtask bootstrap-ledger  (once ever, on a dataset that has never shipped an id)";

/// Per-entry data extracted from `ontology.json` once and reused to emit
/// both the `ontology` and the `rated` modules.
#[derive(Debug, Clone)]
struct EntryRecord {
  /// The AudioSet machine id — the entry's stable upstream identity, the
  /// ledger's join key, and the string the `code` hash is taken over.
  mid: String,
  /// The permanent id assigned by the ledger.
  sound_id: u16,
  const_ident: syn::Ident,
  code: i64,
  name: String,
  /// Alias variants (original casing) — stored on the struct's `aliases` field.
  alias_strings: Vec<String>,
  /// Lowercased phf keys for this entry (mid + alias variants, deduped).
  phf_keys: Vec<String>,
  description: String,
  citation_uri: Option<String>,
  /// Restriction enum tokens (one per restriction string).
  restrictions: Vec<TokenStream>,
  child_ids: Vec<String>,
}

fn main() {
  let mut options = CodegenOptions::default();
  let mut bootstrap = false;
  for arg in std::env::args().skip(1) {
    match arg.as_str() {
      "codegen" => {}
      "bootstrap-ledger" => bootstrap = true,
      "--allow-retire-and-mint" => options.allow_retire_and_mint = true,
      other => {
        eprintln!("unknown xtask argument: {other}");
        eprintln!("{USAGE}");
        std::process::exit(1);
      }
    }
  }

  if bootstrap {
    if options.allow_retire_and_mint {
      eprintln!("bootstrap-ledger takes no options: a genesis run retires nothing.");
      eprintln!("{USAGE}");
      std::process::exit(1);
    }
    bootstrap_ledger();
  } else {
    codegen(options, Genesis::Forbidden);
  }
}

/// Create the permanent-id ledger, once, for a dataset that has never
/// shipped an id.
///
/// A separate command rather than a flag on `codegen`, and that is the
/// guard rather than a matter of taste. As a flag it sat on the normal
/// path, where a LOST ledger is indistinguishable from a never-created
/// one: minting would restart at 1 in the CURRENT ontology order, so
/// after any retirement or upstream reordering the numbers already
/// stored in databases would come back bound to different classes.
/// Nothing downstream could notice — a ledger with no rows retires
/// nothing, so the retire-and-mint gate stays quiet, the write-time seal
/// has only that empty baseline to compare against, and the install
/// succeeds.
///
/// So genesis has to prove it is genesis, twice. The ledger must be
/// ABSENT, not merely empty, because an empty file is a lost ledger
/// wearing a header. And neither generated table may already carry ids:
/// those tables are committed, so an id in one of them has been
/// published and may be stored somewhere.
fn bootstrap_ledger() {
  let paths = CodegenPaths::new(&workspace_root());
  assert_genesis(&paths);
  codegen(CodegenOptions::default(), Genesis::Allowed);
}

/// The two proofs [`bootstrap_ledger`] requires. Split out so the
/// refusals can be exercised against a fixture rather than the live
/// workspace.
fn assert_genesis(paths: &CodegenPaths) {
  assert!(
    !paths.ledger.exists(),
    "{}: the permanent-id ledger already exists, so this dataset is past \
     genesis. If it looks wrong, restore it from version control — never \
     recreate it, because minting from scratch renumbers every class. An \
     empty ledger is a lost ledger, not a new one.",
    paths.ledger.display(),
  );

  for table in [&paths.ontology_out, &paths.rated_out] {
    let Ok(source) = std::fs::read_to_string(table) else {
      continue;
    };
    assert!(
      !source.contains("SoundEventId::new("),
      "{}: this generated table already carries permanent ids, so ids have \
       been emitted from this dataset and may be stored downstream. Genesis \
       would hand those numbers to different classes. Restore the ledger \
       from version control instead.",
      table.display(),
    );
  }
}

fn workspace_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("xtask crate must live in workspace root")
    .to_path_buf()
}

fn codegen(options: CodegenOptions, genesis: Genesis) {
  let CodegenPaths {
    ontology_json,
    rated_csv,
    ledger: ledger_path,
    ontology_out,
    rated_out,
    lock_anchor,
  } = CodegenPaths::new(&workspace_root());

  // 0. Take the transaction lock BEFORE reading anything. Every input
  //    read here is part of the snapshot this run will emit from: an
  //    older run that parsed ontology.json, paused, and only locked
  //    later would install tables built from its stale ontology over a
  //    newer run's, with the ledger rows surviving but the emitted
  //    output regressing. Held until this function returns.
  let _lock = LedgerLock::acquire(&lock_anchor);

  // 1. Parse ontology.json into per-entry records (one pass). Each record
  //    resolves its permanent id against the ledger as it is built: an
  //    existing mid keeps its number, a new one mints above the high-water
  //    mark, and nothing already assigned moves.
  let raw_data = std::fs::read_to_string(&ontology_json)
    .unwrap_or_else(|e| panic!("failed to read {}: {e}", ontology_json.display()));
  let tags: Vec<RawSoundEvent> =
    serde_json::from_str(&raw_data).expect("failed to parse ontology.json");
  let mut ledger = Ledger::load(&ledger_path, genesis);
  let records: Vec<EntryRecord> = tags
    .iter()
    .map(|tag| build_record(tag, &mut ledger))
    .collect();

  // 1b. Persist the ledger before emitting anything, so a run that would
  //     renumber stops before it can write a generated table.
  ledger.assert_rename_resolved(options.allow_retire_and_mint);
  ledger.write(&ledger_path);
  for (id, mid) in &ledger.minted {
    println!("  minted permanent id {id} for {mid:?}");
  }
  for (id, mid) in ledger.retired() {
    println!("  id {id} stays retired (no ontology entry for {mid:?}); never reused");
  }

  // 2. Parse the rated CSV into a set of mids (preserves CSV ordering, but
  //    we only need set membership for filtering).
  let rated_rows = read_rated_rows(&rated_csv);
  let rated_ids = rated_rows
    .iter()
    .map(|row| row.mid.clone())
    .collect::<Vec<_>>();

  // 3. Emit the full ontology module (all 632 records).
  let all_mids: HashSet<&str> = records.iter().map(|r| r.mid.as_str()).collect();
  emit_module(
    &records,
    &all_mids,
    "SoundEvent",
    "UnknownSoundEventCode",
    "ontology",
    &ontology_out,
    None,
  );

  // 4. Emit the rated module (only the 527 entries in the CSV; their child
  //    links are filtered to other rated entries). The ids are the same
  //    ledger's — `rated` is a subset of the ontology keyed on the same
  //    mids, so a class in both views carries one id in each.
  let rated_set: HashSet<&str> = rated_ids.iter().map(String::as_str).collect();
  emit_module(
    &records,
    &rated_set,
    "RatedSoundEvent",
    "UnknownRatedSoundEventCode",
    "rated",
    &rated_out,
    Some(&rated_rows),
  );

  println!("wrote {}", ledger_path.display());
}

fn read_rated_rows(path: &PathBuf) -> Vec<CsvRow> {
  let mut rdr = csv::ReaderBuilder::new()
    .has_headers(true)
    .from_path(path)
    .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()));
  let mut out = Vec::new();
  for row in rdr.deserialize::<CsvRow>() {
    out.push(row.expect("failed to parse CSV row"));
  }
  out.sort_unstable_by_key(|row| row.index);
  for (expected_index, row) in out.iter().enumerate() {
    assert_eq!(
      row.index, expected_index,
      "CSV indices must be contiguous and start from 0; expected {expected_index}, found {} for {}",
      row.index, row.mid
    );
  }
  out
}

fn build_record(tag: &RawSoundEvent, ledger: &mut Ledger) -> EntryRecord {
  let const_ident = id_to_const_name_ident(&tag.id);
  let mid = tag.id.trim().to_string();
  let name = tag.name.trim().to_string();
  // The permanent id, assigned from the ledger and keyed on the same mid the
  // code hashes. Unlike the code it is not derived from the mid at all: a
  // re-midding upstream can be absorbed by editing the ledger row in place,
  // which the hash could never do.
  let sound_id = ledger.id_for(&mid);
  // The code hashes the *id* — the AudioSet MID, which is the entry's stable
  // identity. `name` is the display label, is editable upstream, and is often a
  // comma-joined list (`"Male speech, man speaking"`) that the alias expansion
  // below splits apart; keying off it would rebind the code on any relabelling
  // and orphan every stored row.
  //
  // The hash is 32 bits, widened losslessly to the `i64` the storage-key
  // vocabulary accepts, so every code lands in `0..=u32::MAX`. A 64-bit hash
  // would put roughly half the codes above `i64::MAX`, where they read back as
  // negative literals — `Display` on the `Unknown*Code` errors prints the minus
  // sign and the derived `Ord` inverts for exactly those entries. Pairwise
  // distinctness is asserted per table in `emit_module`; it is not assumed.
  let code = i64::from(xxh32(mid.as_bytes(), CODE_SEED));

  // Alias variants for the struct's `aliases` field — original casing,
  // deduped within the entry by exact-string equality.
  let alias_strings: Vec<String> = tag
    .name
    .split(',')
    .flat_map(|s| {
      let default = s.trim();
      [
        name.clone(),
        default.to_string(),
        default.to_lowercase(),
        default.to_uppercase(),
        default.to_snake_case(),
        default.to_kebab_case(),
        default.to_shouty_snake_case(),
        default.to_shouty_kebab_case(),
        default.TO_SHOUTY_SNEK_CASE(),
        default.to_lower_camel_case(),
        default.to_upper_camel_case(),
        default.to_title_case(),
        default.to_train_case(),
        to_sentence_case(default),
      ]
    })
    .collect::<IndexSet<_>>()
    .into_iter()
    .collect();

  // PHF keys: lowercased forms (because the map is `UncasedStr`-keyed),
  // deduped, including the entry's mid.
  let mut phf_keys: Vec<String> = Vec::new();
  let mut seen = IndexSet::<String>::new();
  for s in std::iter::once(mid.clone()).chain(alias_strings.iter().cloned()) {
    let lower = s.to_lowercase();
    if seen.insert(lower.clone()) {
      phf_keys.push(lower);
    }
  }

  let citation_uri = if tag.citation_uri.trim().is_empty() {
    None
  } else {
    Some(tag.citation_uri.clone())
  };

  let restrictions: Vec<TokenStream> = tag
    .restrictions
    .iter()
    .map(|s| match s.as_str().trim() {
      "abstract" | "ABSTRACT" | "Abstract" => quote! { crate::Restriction::Abstract },
      "blacklist" | "BLACKLIST" | "BlackList" | "blackList" | "Blacklist" => {
        quote! { crate::Restriction::Blacklist }
      }
      other => panic!(
        "unknown restriction `{other}` on entry `{}`; add a new variant to `Restriction` and update xtask",
        tag.id
      ),
    })
    .collect();

  EntryRecord {
    mid,
    sound_id,
    const_ident,
    code,
    name,
    alias_strings,
    phf_keys,
    description: tag.description.clone(),
    citation_uri,
    restrictions,
    child_ids: tag.child_ids.clone(),
  }
}

/// Emit one generated.rs for a module (`ontology` or `rated`).
///
/// Only entries whose mid is in `included_mids` are emitted. The `children`
/// field of each emitted entry is filtered to references that are also in
/// `included_mids`, so traversing the hierarchy stays inside the module.
fn emit_module(
  records: &[EntryRecord],
  included_mids: &HashSet<&str>,
  type_name: &str,
  err_name: &str,
  module_name: &str,
  output_path: &PathBuf,
  rated_rows: Option<&[CsvRow]>,
) {
  let type_ident = format_ident!("{}", type_name);
  let err_ident = format_ident!("{}", err_name);

  let mut consts = Vec::new();
  let mut events = Vec::new();
  let mut from_code_arms = Vec::new();
  let mut from_index_arms = Vec::new();
  // alias_to_consts: lowercased phf key -> set of const idents pointing at it.
  let mut alias_to_consts: IndexMap<String, IndexSet<syn::Ident>> = IndexMap::new();
  // code -> the mid that claimed it, so a hash collision names both entries.
  // Scoped to this module: `ontology` and `rated` are two independent types
  // with two independent `from_code` tables, so a code shared across them is
  // not a collision — and because `rated` is a subset of `ontology` and both
  // hash the same mid, a shared entry deliberately gets the same code in both.
  let mut claimed_codes: HashMap<i64, &str> = HashMap::new();
  // Permanent id -> const ident, for this module's dense `BY_ID` table. The
  // ids come from one ledger spanning the full ontology, so `rated`'s table
  // is the same id space with the unrated slots left `None`.
  let mut ids: Vec<(u16, syn::Ident)> = Vec::new();
  let rated_indices = rated_rows.map(|rows| {
    rows
      .iter()
      .map(|row| (row.mid.as_str(), row.index))
      .collect::<IndexMap<_, _>>()
  });

  for record in records {
    if !included_mids.contains(record.mid.as_str()) {
      continue;
    }
    let const_name_ident = &record.const_ident;
    let mid = &record.mid;
    let sound_id = record.sound_id;
    let name = &record.name;
    let code = record.code;
    if let Some(previous) = claimed_codes.insert(code, mid.as_str()) {
      panic!(
        "sound-event code collision in the `{module_name}` table: mids `{previous}` and `{mid}` \
         both hash to {code}.\nDo not reseed or swap the hash to clear this — that rotates every \
         code in both tables and orphans every code already stored downstream. Pin an explicit \
         override for exactly one of the two mids instead, leaving every other code untouched."
      );
    }
    ids.push((sound_id, const_name_ident.clone()));
    let desp = &record.description;
    let citation_uri = match &record.citation_uri {
      Some(url) => quote! { ::core::option::Option::Some(#url) },
      None => quote! { ::core::option::Option::None },
    };
    let aliases = record.alias_strings.iter().map(|s| s.as_str());
    let restrictions = record.restrictions.iter();
    let rated_index = rated_indices
      .as_ref()
      .and_then(|indices| indices.get(record.mid.as_str()))
      .copied();
    let rated_index_field = rated_index.map(|index| quote! { index: #index, });
    // Filter children to those still inside the included set.
    let children = record
      .child_ids
      .iter()
      .filter(|c| included_mids.contains(c.as_str()))
      .map(|c| id_to_const_name_ident(c));

    consts.push(quote! {
      const #const_name_ident: &super::#type_ident = &super::#type_ident {
        id: crate::SoundEventId::new(#sound_id),
        code: #code,
        mid: #mid,
        name: #name,
        aliases: &[#(#aliases),*],
        description: #desp,
        citation_uri: #citation_uri,
        children: &[#(#children),*],
        restrictions: &[#(#restrictions),*],
        #rated_index_field
      };
    });
    events.push(const_name_ident.clone());

    from_code_arms.push(quote! {
      #code => #const_name_ident
    });

    for key in &record.phf_keys {
      alias_to_consts
        .entry(key.clone())
        .or_default()
        .insert(const_name_ident.clone());
    }
  }

  if let Some(rows) = rated_rows {
    events = rows
      .iter()
      .map(|row| {
        assert!(
          included_mids.contains(row.mid.as_str()),
          "rated CSV entry {} is missing from the generated {} module",
          row.mid,
          module_name
        );
        id_to_const_name_ident(&row.mid)
      })
      .collect();

    from_index_arms = rows
      .iter()
      .map(|row| {
        let index = row.index;
        let const_ident = id_to_const_name_ident(&row.mid);
        quote! {
          #index => #const_ident
        }
      })
      .collect();
  }

  // Build the dense permanent-id → entry table backing `from_id`, so the
  // lookup is a bounds check plus one load. Slot 0 is always `None` (0 is
  // never assigned) and so is every id this module does not carry: a
  // retired one, or — in `rated` — one held by an ontology-only class.
  let max_id = ids
    .iter()
    .map(|(id, _)| *id)
    .max()
    .unwrap_or_else(|| panic!("the `{module_name}` table is empty"));
  let mut by_id: Vec<TokenStream> =
    vec![quote! { ::core::option::Option::None }; usize::from(max_id) + 1];
  for (id, ident) in &ids {
    by_id[usize::from(*id)] = quote! { ::core::option::Option::Some(#ident) };
  }
  let by_id_doc = format!(
    " Permanent id → entry, indexed by `SoundEventId::get`. Dense over \
     `0..={max_id}`: slot 0 holds `None` because 0 is never assigned, and so \
     does every id the `{module_name}` view does not carry — one retired from \
     the dataset, or one held by a class this view excludes. Backs \
     `{type_name}::from_id`, the reverse half of the id bijection. Generated \
     by `cargo xtask codegen` from `assets/sound_ids.csv`; do not edit by hand."
  );

  // Build the perfect-hash map with phf_codegen, keyed by &UncasedStr so
  // lookups are case-insensitive.
  let mut phf_map = phf_codegen::Map::<&UncasedStr>::new();
  let value_strings: Vec<(String, String)> = alias_to_consts
    .iter()
    .map(|(key, idents)| {
      let inner = idents
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(", ");
      (key.clone(), format!("__slice(&[{inner}])"))
    })
    .collect();
  for (key, value) in &value_strings {
    phf_map.entry(UncasedStr::new(key.as_str()), value);
  }
  let phf_built = phf_map.build().to_string();

  // The handcoded chunk of generated.rs (consts + helper + impls). The phf
  // static is appended afterwards as raw text from `phf_built`.
  let events_doc = format!(" Returns a slice of all possible events of `{type_name}`.");
  let from_index_impl = if rated_rows.is_some() {
    quote! {
      /// Get an entry by its model output index, if it exists.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn from_index(index: usize) -> ::core::option::Option<&'static Self> {
        ::core::option::Option::Some(match index {
          #(#from_index_arms),*,
          _ => return ::core::option::Option::None,
        })
      }
    }
  } else {
    quote! {}
  };

  let body = quote! {
    #(#consts)*

    #[doc = #by_id_doc]
    pub(super) static BY_ID: &[::core::option::Option<&'static super::#type_ident>] = &[
      #(#by_id),*
    ];

    #[doc(hidden)]
    const fn __slice(
      s: &'static [&'static super::#type_ident],
    ) -> &'static [&'static super::#type_ident] {
      s
    }

    const _: () = {
      use super::{#type_ident, #err_ident};

      impl ::core::convert::TryFrom<i64> for &'static #type_ident {
        type Error = #err_ident;

        #[cfg_attr(not(tarpaulin), inline(always))]
        fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
          #type_ident::from_code(value).ok_or(#err_ident(value))
        }
      }

      impl ::core::convert::TryFrom<i64> for #type_ident {
        type Error = #err_ident;

        #[cfg_attr(not(tarpaulin), inline(always))]
        fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
          <&'static #type_ident>::try_from(value).cloned()
        }
      }

      impl #type_ident {
        /// Get an entry by its code, if it exists.
        #[cfg_attr(not(tarpaulin), inline(always))]
        pub const fn from_code(id: ::core::primitive::i64) -> ::core::option::Option<&'static Self> {
          ::core::option::Option::Some(match id {
            #(#from_code_arms),*,
            _ => return ::core::option::Option::None,
          })
        }

        #from_index_impl

        /// Get all entries matching a mid, name, or alias.
        ///
        /// Lookups are case-insensitive: `"man speaking"`, `"MAN SPEAKING"`,
        /// and `"Man Speaking"` all resolve to the same entry. Separator
        /// styles (`"man_speaking"`, `"man-speaking"`, `"manSpeaking"`) are
        /// each indexed separately.
        ///
        /// Returns an empty slice if no entries match. Most names map to a
        /// single entry, but ambiguous aliases (e.g. `"Inside"`) may return
        /// multiple entries.
        #[cfg_attr(not(tarpaulin), inline(always))]
        pub fn from_key(name: &str) -> &'static [&'static Self] {
          match DATASET.get(::uncased::UncasedStr::new(name)) {
            ::core::option::Option::Some(slice) => slice,
            ::core::option::Option::None => &[],
          }
        }

        #[doc = #events_doc]
        #[cfg_attr(not(tarpaulin), inline(always))]
        pub const fn events() -> &'static [&'static Self] {
          const EVENTS: &[&super::#type_ident] = &[#(#events),*];

          EVENTS
        }
      }
    };
  };

  let body_pretty = prettyplease::unparse(&syn::File::parse.parse2(body).unwrap());

  let phf_static = format!(
    "use ::uncased::UncasedStr;\n\n\
     /// All {module_name} entries, indexed by mid, name, and alias.\n\
     ///\n\
     /// Lookups are case-insensitive (the keys are [`UncasedStr`]), so any\n\
     /// case form of an alias resolves through the same bucket. Each key\n\
     /// maps to a slice of all entries that share that name or alias —\n\
     /// most keys map to a single entry, but a few ambiguous aliases\n\
     /// (e.g. `\"Inside\"`) may map to multiple entries.\n\
     pub(super) static DATASET: ::phf::Map<&'static UncasedStr, &'static [&'static super::{type_name}]> = {phf_built};\n",
  );

  let output = format!(
    "\n\n// This file is generated by `cargo xtask codegen`, do not edit it manually.\n\n{body_pretty}\n{phf_static}\n",
  );

  std::fs::write(output_path, output)
    .unwrap_or_else(|e| panic!("failed to write {}: {e}", output_path.display()));
  println!(
    "wrote {} ({} entries, ids 1..={max_id})",
    output_path.display(),
    events.len(),
  );
}

#[inline]
fn id_to_const_name(id: &str) -> String {
  id.replace('/', "_").to_uppercase()
}

#[inline]
fn id_to_const_name_ident(id: &str) -> syn::Ident {
  format_ident!("{}", id_to_const_name(id))
}

/// Sentence case: lowercase the whole string, then uppercase the first character.
/// For `"man speaking"` this yields `"Man speaking"`. heck has no built-in for
/// this style — `to_title_case` would produce `"Man Speaking"` instead.
fn to_sentence_case(s: &str) -> String {
  let lower = s.to_lowercase();
  let mut chars = lower.chars();
  match chars.next() {
    Some(first) => first.to_uppercase().chain(chars).collect(),
    None => String::new(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Build a ledger from `(id, mid)` pairs, as if read from the file.
  fn ledger(rows: &[(u16, &str)]) -> Ledger {
    Ledger::from_rows(
      rows
        .iter()
        .map(|&(id, mid)| LedgerRow {
          id,
          mid: mid.to_string(),
        })
        .collect(),
      "test ledger",
    )
  }

  /// Resolve a run's worth of ontology mids against a ledger, in order.
  /// A throwaway workspace root laid out the way `CodegenPaths` expects,
  /// so the genesis proofs can be exercised without touching the live
  /// ledger or the committed tables.
  fn fixture_paths(root: &Path) -> CodegenPaths {
    let paths = CodegenPaths::new(root);
    for dir in [
      paths.ledger.parent(),
      paths.ontology_out.parent(),
      paths.rated_out.parent(),
    ]
    .into_iter()
    .flatten()
    {
      std::fs::create_dir_all(dir).expect("fixture dir");
    }
    paths
  }

  fn resolve(ledger: &mut Ledger, mids: &[&str]) -> Vec<u16> {
    mids.iter().map(|mid| ledger.id_for(mid)).collect()
  }

  #[test]
  fn existing_mids_keep_their_ids() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz"), (3, "/t/dd00012")]);
    assert_eq!(
      resolve(&mut l, &["/m/09x0r", "/m/05zppz", "/t/dd00012"]),
      [1, 2, 3]
    );
    assert!(l.minted.is_empty(), "nothing new should have been minted");
    assert!(l.retired().is_empty());
  }

  /// `ontology.json` order must not touch the assignment — the ledger is
  /// keyed by mid, not by position.
  #[test]
  fn reordered_input_does_not_renumber() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz"), (3, "/t/dd00012")]);
    assert_eq!(
      resolve(&mut l, &["/t/dd00012", "/m/09x0r", "/m/05zppz"]),
      [3, 1, 2]
    );
    assert!(l.minted.is_empty());
  }

  #[test]
  fn a_new_mid_mints_above_the_high_water_mark() {
    let mut l = ledger(&[(1, "/m/09x0r"), (7, "/m/05zppz")]);
    assert_eq!(
      resolve(&mut l, &["/m/09x0r", "/m/05zppz", "/t/dd00099"]),
      [1, 7, 8]
    );
    assert_eq!(l.minted, vec![(8, "/t/dd00099".to_string())]);
  }

  /// The core promise. A class retires while holding the highest id; its
  /// row stays; the next newcomer must NOT receive that number.
  #[test]
  fn a_retired_high_water_id_is_never_reminted() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz"), (3, "/t/dd00012")]);
    // `/t/dd00012` (id 3, the high-water mark) is gone from the ontology.
    assert_eq!(
      resolve(&mut l, &["/m/09x0r", "/m/05zppz", "/t/dd00099"]),
      [1, 2, 4]
    );
    assert_eq!(l.retired(), vec![(3, "/t/dd00012")]);
    assert_eq!(l.minted, vec![(4, "/t/dd00099".to_string())]);
    assert!(
      l.to_csv().contains("3,/t/dd00012"),
      "the retired row must survive the rewrite; it is the only record \
       that 3 was ever handed out",
    );
  }

  /// Falsification of the clause above: this is what goes wrong when a
  /// retired row is deleted from the ledger by hand. Nothing in the
  /// generator can detect it — the tombstone IS the memory — which is
  /// why `tests/ids.rs` pins every ledger row, retired ones included.
  #[test]
  fn deleting_a_tombstone_lets_its_id_be_reminted() {
    let mut without_tombstone = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    assert_eq!(
      resolve(
        &mut without_tombstone,
        &["/m/09x0r", "/m/05zppz", "/t/dd00099"]
      ),
      [1, 2, 3],
      "with /t/dd00012's row deleted, 3 is handed to a different class",
    );

    let mut with_tombstone = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz"), (3, "/t/dd00012")]);
    assert_eq!(
      resolve(
        &mut with_tombstone,
        &["/m/09x0r", "/m/05zppz", "/t/dd00099"]
      ),
      [1, 2, 4],
      "with the row kept, the same input mints 4 instead",
    );
  }

  /// A class that leaves an ontology revision and later comes back is the
  /// same class, so it gets its original id back rather than a fresh one.
  #[test]
  fn a_returning_mid_recovers_its_original_id() {
    let mut gone = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    assert_eq!(resolve(&mut gone, &["/m/09x0r"]), [1]);
    assert_eq!(gone.retired(), vec![(2, "/m/05zppz")]);

    let mut back = Ledger::from_rows(
      // The ledger as the run above would have rewritten it.
      vec![
        LedgerRow {
          id: 1,
          mid: "/m/09x0r".to_string(),
        },
        LedgerRow {
          id: 2,
          mid: "/m/05zppz".to_string(),
        },
      ],
      "test ledger",
    );
    assert_eq!(resolve(&mut back, &["/m/09x0r", "/m/05zppz"]), [1, 2]);
    assert!(back.minted.is_empty(), "/m/05zppz must not be re-minted");
  }

  /// A re-midding reaches the generator as a retirement plus a mint, and
  /// minting would break every stored id for that class. Codegen must
  /// refuse rather than guess.
  #[test]
  #[should_panic(expected = "both retires and mints")]
  fn a_re_midding_is_refused_until_the_operator_resolves_it() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut l, &["/m/09x0r", "/m/05zppzz"]);
    l.assert_rename_resolved(false);
  }

  /// Resolving it the documented way — editing the mid in place, keeping
  /// the id — makes the pairing disappear.
  #[test]
  fn editing_the_mid_in_place_preserves_the_id_and_clears_the_gate() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppzz")]);
    assert_eq!(resolve(&mut l, &["/m/09x0r", "/m/05zppzz"]), [1, 2]);
    assert!(l.minted.is_empty());
    assert!(l.retired().is_empty());
    l.assert_rename_resolved(false);
  }

  /// A genuinely unrelated class swap is allowed, but only with the
  /// operator saying so.
  #[test]
  fn unrelated_retire_and_mint_passes_with_the_flag() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut l, &["/m/09x0r", "/t/dd00099"]);
    l.assert_rename_resolved(true);
  }

  /// Retiring alone is unambiguous — no gate.
  #[test]
  fn retiring_without_minting_is_unambiguous() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut l, &["/m/09x0r"]);
    l.assert_rename_resolved(false);
  }

  /// Minting alone is unambiguous — no gate.
  #[test]
  fn minting_without_retiring_is_unambiguous() {
    let mut l = ledger(&[(1, "/m/09x0r")]);
    resolve(&mut l, &["/m/09x0r", "/t/dd00099"]);
    l.assert_rename_resolved(false);
  }

  #[test]
  #[should_panic(expected = "assigned twice")]
  fn a_duplicate_id_is_rejected() {
    ledger(&[(1, "/m/09x0r"), (1, "/m/05zppz")]);
  }

  #[test]
  #[should_panic(expected = "appears twice")]
  fn a_duplicate_mid_is_rejected() {
    ledger(&[(1, "/m/09x0r"), (2, "/m/09x0r")]);
  }

  #[test]
  #[should_panic(expected = "id 0 is reserved")]
  fn the_reserved_id_zero_is_rejected() {
    ledger(&[(0, "/m/09x0r")]);
  }

  #[test]
  #[should_panic(expected = "exhausted u16")]
  fn exhausting_the_id_space_is_a_hard_error() {
    let mut l = ledger(&[(u16::MAX, "/m/the_last_class")]);
    resolve(&mut l, &["/m/the_last_class", "/m/one_too_many"]);
  }

  /// A mid needing CSV quoting must survive a write/read round trip. No
  /// AudioSet mid contains a comma or a quote today, but the ledger's
  /// parser and its writer have to agree over whatever upstream emits —
  /// a disagreement would corrupt the join key, and the join key is what
  /// holds an id to its class.
  #[test]
  fn a_mid_needing_quoting_round_trips() {
    let awkward = "/m/odd, \"quoted\"";
    let mut l = ledger(&[(1, "/m/09x0r")]);
    assert_eq!(resolve(&mut l, &["/m/09x0r", awkward]), [1, 2]);

    let serialized = l.to_csv();
    let mut rdr = csv::ReaderBuilder::new()
      .has_headers(true)
      .comment(Some(b'#'))
      .from_reader(serialized.as_bytes());
    let rows: Vec<LedgerRow> = rdr
      .deserialize::<LedgerRow>()
      .map(|r| r.expect("re-read own output"))
      .collect();
    let reread = Ledger::from_rows(rows, "round trip");
    assert_eq!(reread.by_mid.get(awkward).copied(), Some(2));
    assert_eq!(reread.high_water, 2);
  }

  /// The serialized form is byte-stable: same ledger, same bytes, with
  /// rows in id order regardless of insertion order.
  #[test]
  fn serialization_is_deterministic_and_id_ordered() {
    let mut a = ledger(&[(2, "/m/05zppz"), (1, "/m/09x0r")]);
    let mut b = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut a, &["/m/09x0r", "/m/05zppz"]);
    resolve(&mut b, &["/m/05zppz", "/m/09x0r"]);
    assert_eq!(a.to_csv(), b.to_csv());

    let body = a.to_csv();
    let first = body.find("1,/m/09x0r").expect("first row");
    let second = body.find("2,/m/05zppz").expect("second row");
    assert!(first < second, "rows must be sorted by id");
    assert!(body.ends_with('\n'));
    assert!(!body.contains('\r'), "the ledger is always LF");
  }

  /// A missing ledger is a hard error: it is the only record of retired
  /// ids, so regenerating it from `ontology.json` would remint them.
  #[test]
  #[should_panic(expected = "permanent-id ledger is missing")]
  fn a_missing_ledger_is_refused_without_the_bootstrap_flag() {
    Ledger::load(Path::new("/nonexistent/sound_ids.csv"), Genesis::Forbidden);
  }

  /// An emptied ledger — header present, no rows — resets the high-water
  /// mark and remints everything from 1, and because it retires nothing
  /// the re-midding gate stays silent.
  ///
  /// It has to be refused EVEN UNDER GENESIS, which is the mode that
  /// would actually do the reminting: a file that exists but holds no
  /// rows is a lost ledger wearing a header, and genesis requires
  /// absence. Both modes are checked here so no future flag can quietly
  /// become an override.
  #[test]
  fn an_empty_ledger_is_refused_in_every_mode() {
    let dir = std::env::temp_dir().join(format!("sedid-empty-ledger-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("sound_ids.csv");
    std::fs::write(&path, "# prologue\nid,mid\n").expect("write empty ledger");

    let refusals = [Genesis::Forbidden, Genesis::Allowed].map(|genesis| {
      std::panic::catch_unwind(|| Ledger::load(&path, genesis))
        .err()
        .map(|payload| {
          let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
          assert!(
            message.contains("has no rows"),
            "refused for the wrong reason: {message}"
          );
        })
    });
    std::fs::remove_dir_all(&dir).ok();

    assert!(
      refusals.iter().all(Option::is_some),
      "an empty ledger was accepted in at least one mode"
    );
  }

  /// Genesis must refuse once the ledger exists at all — the case where
  /// a published ledger was emptied or is being recreated by hand.
  #[test]
  #[should_panic(expected = "past genesis")]
  fn genesis_refuses_when_a_ledger_already_exists() {
    let dir = std::env::temp_dir().join(format!("sedid-genesis-exists-{}", std::process::id()));
    let paths = fixture_paths(&dir);
    std::fs::write(&paths.ledger, "# prologue\nid,mid\n1,/m/09x0r\n").expect("ledger");
    let result = std::panic::catch_unwind(|| assert_genesis(&paths));
    std::fs::remove_dir_all(&dir).ok();
    match result {
      Ok(()) => panic!("genesis was allowed on a dataset that already has a ledger"),
      Err(payload) => std::panic::resume_unwind(payload),
    }
  }

  /// Genesis must refuse when a generated table already carries ids,
  /// even with the ledger gone. That is the dangerous shape: the ledger
  /// was LOST, the ids are already published in the committed tables,
  /// and reminting from 1 would rebind them to other classes.
  #[test]
  #[should_panic(expected = "already carries permanent ids")]
  fn genesis_refuses_when_a_generated_table_already_carries_ids() {
    let dir = std::env::temp_dir().join(format!("sedid-genesis-ids-{}", std::process::id()));
    let paths = fixture_paths(&dir);
    // No ledger at all, but the committed tables hold ids.
    std::fs::write(
      &paths.rated_out,
      "const SPEECH: &super::RatedSoundEvent = &super::RatedSoundEvent { \
       id: crate::SoundEventId::new(3u16), };",
    )
    .expect("table");
    let result = std::panic::catch_unwind(|| assert_genesis(&paths));
    std::fs::remove_dir_all(&dir).ok();
    match result {
      Ok(()) => panic!("genesis was allowed while published ids exist"),
      Err(payload) => std::panic::resume_unwind(payload),
    }
  }

  /// And it must still permit a genuinely fresh dataset: no ledger, and
  /// no table carrying ids.
  #[test]
  fn genesis_is_allowed_on_a_dataset_that_has_never_shipped_an_id() {
    let dir = std::env::temp_dir().join(format!("sedid-genesis-fresh-{}", std::process::id()));
    let paths = fixture_paths(&dir);
    assert_genesis(&paths);
    std::fs::write(&paths.ontology_out, "// no ids emitted yet").expect("table");
    assert_genesis(&paths);
    std::fs::remove_dir_all(&dir).ok();
  }

  #[test]
  fn bootstrap_creates_an_empty_ledger() {
    let l = Ledger::load(Path::new("/nonexistent/sound_ids.csv"), Genesis::Allowed);
    assert_eq!(l.high_water, 0);
    assert!(l.by_mid.is_empty());
  }

  /// Bootstrapping mints from 1, never 0.
  #[test]
  fn bootstrap_mints_from_one() {
    let mut l = Ledger::load(Path::new("/nonexistent/sound_ids.csv"), Genesis::Allowed);
    assert_eq!(resolve(&mut l, &["/m/09x0r", "/m/05zppz"]), [1, 2]);
  }

  /// The write-time seal: a code path that moved an already-assigned id
  /// must be caught before the ledger reaches disk, not after.
  #[test]
  #[should_panic(expected = "ids are PERMANENT")]
  fn write_refuses_to_move_an_already_assigned_id() {
    let mut l = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut l, &["/m/09x0r", "/m/05zppz"]);
    // Simulate a future edit to this type mis-assigning an existing mid.
    l.by_mid.insert("/m/09x0r".to_string(), 9);
    l.write(Path::new("/nonexistent/sound_ids.csv"));
  }

  /// An install that fails partway must leave the ledger byte-identical.
  ///
  /// A TRUNCATED ledger is worse than a missing one: it parses, it still
  /// looks dense, and because the rows are id-ordered it drops the
  /// HIGHEST ids — lowering the high-water mark so spent numbers are
  /// minted again, with `load`, the seal and the retire-and-mint gate all
  /// silent. Blocking the staging path with a directory fails the install
  /// at `File::create`, which is exactly where a truncating writer would
  /// already have destroyed the original.
  #[test]
  fn a_failed_install_leaves_the_ledger_byte_identical() {
    let dir = std::env::temp_dir().join(format!("sedid-failed-install-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("sound_ids.csv");

    let mut first = ledger(&[(1, "/m/09x0r"), (2, "/m/05zppz")]);
    resolve(&mut first, &["/m/09x0r", "/m/05zppz"]);
    first.write(&path);
    let before = std::fs::read(&path).expect("ledger installed");

    // A second run that mints a third id, with its staging file blocked.
    let mut next = Ledger::load(&path, Genesis::Forbidden);
    resolve(&mut next, &["/m/09x0r", "/m/05zppz", "/t/dd00099"]);
    std::fs::create_dir_all(Ledger::staging_path(&path)).expect("block the staging path");
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next.write(&path)));

    let after = std::fs::read(&path).expect("the ledger must still be there");
    std::fs::remove_dir_all(&dir).ok();

    assert!(
      failed.is_err(),
      "a blocked staging path must fail the run, not report success"
    );
    assert_eq!(
      after, before,
      "a failed install must leave the ledger exactly as it was"
    );
  }

  /// A completed install replaces the whole file and leaves no staging
  /// file behind — an untracked leftover in `assets/` fails CI's
  /// `codegen-up-to-date` job.
  #[test]
  fn a_completed_install_leaves_no_staging_file() {
    let dir = std::env::temp_dir().join(format!("sedid-clean-install-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("sound_ids.csv");

    let mut l = ledger(&[(1, "/m/09x0r")]);
    resolve(&mut l, &["/m/09x0r", "/t/dd00099"]);
    l.write(&path);

    let staged_survived = Ledger::staging_path(&path).exists();
    let reread = Ledger::load(&path, Genesis::Forbidden);
    let mut leftovers: Vec<String> = std::fs::read_dir(&dir)
      .expect("scratch dir")
      .filter_map(|entry| entry.ok())
      .map(|entry| entry.file_name().to_string_lossy().into_owned())
      .collect();
    leftovers.sort();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
      !staged_survived,
      "the staging file must not survive a successful install"
    );
    assert_eq!(
      leftovers,
      vec!["sound_ids.csv".to_string()],
      "only the ledger may remain beside it"
    );
    assert_eq!(
      reread.by_mid, l.by_mid,
      "the installed ledger must reread whole"
    );
    assert_eq!(reread.high_water, l.high_water);
  }

  /// Two runs that read the same ledger and mint from the same
  /// high-water mark must not both install. The one that finishes last
  /// would otherwise write a ledger that never saw the other's
  /// assignment, deleting a successfully minted id and freeing that
  /// number to be handed to a different class later. The stale run has
  /// to refuse, not win.
  #[test]
  fn a_stale_snapshot_refuses_to_install_over_a_newer_ledger() {
    let dir = std::env::temp_dir().join(format!("sedid-stale-snapshot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("sound_ids.csv");

    let mut seed = ledger(&[(1, "/m/09x0r")]);
    resolve(&mut seed, &["/m/09x0r"]);
    seed.write(&path);

    // Both runs read the same snapshot, then diverge: each mints a
    // different new mid, and both get id 2.
    let mut first = Ledger::load(&path, Genesis::Forbidden);
    let mut second = Ledger::load(&path, Genesis::Forbidden);
    assert_eq!(resolve(&mut first, &["/m/09x0r", "/t/dd00001"]), [1, 2]);
    assert_eq!(resolve(&mut second, &["/m/09x0r", "/t/dd00002"]), [1, 2]);

    first.write(&path);
    let after_first = std::fs::read(&path).expect("first install");
    let clobbered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| second.write(&path)));
    let after_second = std::fs::read(&path).expect("ledger still there");
    let installed = Ledger::load(&path, Genesis::Forbidden);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
      clobbered.is_err(),
      "the stale run must refuse to install over a newer ledger"
    );
    assert_eq!(
      after_second, after_first,
      "the stale run must leave the newer ledger exactly as it found it"
    );
    assert_eq!(
      installed.by_mid.get("/t/dd00001").copied(),
      Some(2),
      "the id the first run assigned must survive"
    );
    assert!(
      !installed.by_mid.contains_key("/t/dd00002"),
      "the stale run must not have landed its own mint"
    );
  }

  /// The transaction lock must actually exclude, and the proof must not
  /// depend on scheduling: a sleep-ordered or channel-ordered test still
  /// passes with the lock removed whenever the contender happens not to
  /// run inside the holder's window.
  ///
  /// So there is no window and no second thread. While the lock is held,
  /// a second acquisition must report that it would block; once it is
  /// released, the same acquisition must succeed. Both are checked
  /// synchronously, so removing the lock fails the first assertion every
  /// time rather than most of the time.
  #[test]
  fn the_transaction_lock_excludes_a_second_holder() {
    let dir = std::env::temp_dir().join(format!("sedid-lock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let anchor = dir.join("codegen.lock");
    std::fs::write(&anchor, b"anchor").expect("anchor");

    let held = LedgerLock::acquire(&anchor);
    assert!(
      LedgerLock::try_acquire(&anchor).is_none(),
      "a second holder got the lock while it was held"
    );
    drop(held);
    assert!(
      LedgerLock::try_acquire(&anchor).is_some(),
      "the lock was not released"
    );

    std::fs::remove_dir_all(&dir).ok();
  }

  /// The lock anchor must be a file codegen never opens — and this has
  /// to be checked as an IDENTITY, not by doing I/O.
  ///
  /// Windows file locks are MANDATORY: a locked input cannot be read and
  /// a locked OUTPUT cannot be written, not even by the process holding
  /// the lock. Reading the inputs back is therefore not enough of a
  /// check, because codegen also writes both `generated.rs` files while
  /// still holding the lock — an anchor moved onto one of those would
  /// pass every read-based test, pass the Ubuntu-only codegen job (unix
  /// locks are advisory), and still break every Windows run, possibly
  /// after the ledger had already been installed.
  ///
  /// So: while the real anchor is held, a second lock on any OTHER file
  /// must be free, and a second lock on the anchor itself must not be.
  /// That distinguishes "same file" from "different file" on advisory
  /// and mandatory platforms alike, without doing anything a locked file
  /// would refuse. The paths come from [`CodegenPaths`] rather than a
  /// list copied here, so a file codegen gains later is covered without
  /// this test being touched.
  #[test]
  fn the_lock_anchor_is_no_file_codegen_reads_or_writes() {
    let paths = CodegenPaths::new(&workspace_root());
    let held = LedgerLock::acquire(&paths.lock_anchor);

    // Negative control: the identity check can actually tell the anchor
    // apart from another file, so the assertions below mean something.
    assert!(
      LedgerLock::try_acquire(&paths.lock_anchor).is_none(),
      "the identity check cannot distinguish the held anchor from a free file"
    );

    for path in paths.read_and_written() {
      assert!(
        LedgerLock::try_acquire(path).is_some(),
        "{} IS the lock anchor. Codegen reads or writes it while holding \
         the transaction lock, and Windows locks are mandatory, so that \
         fails every Windows run. Anchor the lock on a file nothing opens.",
        path.display()
      );
      // And it is still reachable the way codegen reaches it.
      let bytes = std::fs::read(path).unwrap_or_else(|e| {
        panic!(
          "{} is unreadable while the lock is held: {e}",
          path.display()
        )
      });
      assert!(!bytes.is_empty(), "{} read back empty", path.display());
    }

    drop(held);
  }
}
