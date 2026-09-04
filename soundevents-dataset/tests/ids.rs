//! Permanent sound-event ids: the [`SoundEventId`] bijection and the
//! canaries that pin the whole assignment.
//!
//! Downstream stores a `SoundEventId` and resolves it back later, so these
//! ids are load-bearing in a way the rest of the tables are not: a
//! renumbering does not fail a build or a lookup, it silently repoints
//! every stored id at a different sound event. Four things guard that
//! here.
//!
//! 1. **The bijection** — `from_id(e.id())` is `e` for every entry, ids
//!    are unique, and `from_id` is total over `u16`: an id a view does not
//!    carry answers `None` rather than a neighbour.
//! 2. **The table pins** — [`ONTOLOGY_FINGERPRINT`] and
//!    [`RATED_FINGERPRINT`] hash the complete `(id, mid)` assignment each
//!    view ships. Any renumbering changes them, and the cross-checks prove
//!    the tables still match the committed `assets/sound_ids.csv`.
//! 3. **The ledger pin** — [`LEDGER_FINGERPRINT`] hashes every ledger row,
//!    *retired ones included*. Those are invisible to the table pins, and
//!    losing one un-burns its id: the generator's high-water mark drops
//!    and the next new class is minted that number. Nothing else notices,
//!    because the tables, the generated files and the codegen diff all
//!    agree with the shortened ledger.
//! 4. **The probes** — `renumber_probe_trips_the_pin` and
//!    `tombstone_loss_trips_the_ledger_pin` break the assignment on
//!    purpose and assert the pins move, so neither can pass by being
//!    insensitive to what it claims to catch.
//!
//! Two table pins where the shape twin (`colorthief-dataset`) has one,
//! because this crate ships two feature-gated *views* of a single id
//! space: the full ontology and the rated subset. Each is pinned under its
//! own feature, and [`rated_ids_agree_with_the_ontology`] pins the
//! relationship between them — a class in both views must carry the same
//! id in each, which is the property a per-view ledger would lose.

use std::collections::BTreeMap;

use soundevents_dataset::SoundEventId;

/// The committed permanent-id ledger, the authority `cargo xtask codegen`
/// assigns from.
const LEDGER: &str = include_str!("../assets/sound_ids.csv");

/// FNV-1a 64 over **every row of the ledger**, live and retired.
///
/// The table pins cannot see a retired row: they hash a shipped table, and
/// a retired class is by definition not in one. But a retired row is the
/// only record that its number was ever handed out — delete it and the
/// generator's high-water mark drops, so the next new class is minted that
/// number and every id stored for the old class silently resolves to the
/// new one. Nothing else catches that: not the tables, not the generated
/// files, not the codegen diff, because all three agree with the shortened
/// ledger.
///
/// So this pins the tombstones. Update it for a mint, a retirement, or a
/// mid corrected in place — never for a row that *disappeared*.
///
/// It equals [`ONTOLOGY_FINGERPRINT`] today, and that is not a
/// copy-paste: nothing has retired yet, so the ledger's rows are exactly
/// the ontology table's. The first retirement separates them forever.
/// [`tombstone_loss_trips_the_ledger_pin`] asserts that relationship
/// rather than leaving it to coincidence.
const LEDGER_FINGERPRINT: u64 = 0xa285_5ed4_8e47_3e1d;

/// FNV-1a 64 over the complete `(id, mid)` assignment the `ontology` view
/// ships — see [`fingerprint`].
///
/// **This number changing means the dataset's identity changed.** Ids are
/// permanent: the only legitimate reasons to update it are a new class
/// minting a fresh id, a class retiring, or an upstream correction to a
/// class's *mid* applied in place in the ledger. Before touching it,
/// confirm from the `assets/sound_ids.csv` diff that no existing id moved
/// to a different class — that would break every id already stored
/// downstream, and no rebuild would say so.
#[cfg(feature = "ontology")]
const ONTOLOGY_FINGERPRINT: u64 = 0xa285_5ed4_8e47_3e1d;

/// The same, for the `rated` view. It is a subset of the ontology drawn
/// from the same ledger, so its pin moves for a change to any of the 527
/// released classes and stays put for a change confined to the 105
/// ontology-only ones.
#[cfg(feature = "rated")]
const RATED_FINGERPRINT: u64 = 0x4763_c3b6_b574_db62;

/// Rows in the ledger — every id ever assigned, live or retired. Only ever
/// grows.
const EXPECTED_LEDGER_ROWS: usize = 632;

/// Entries in each view today. Pinned alongside the fingerprints so a
/// failure reads as a summary before the reader reaches the hash.
#[cfg(feature = "ontology")]
const EXPECTED_ONTOLOGY_ENTRIES: usize = 632;
/// See [`EXPECTED_ONTOLOGY_ENTRIES`].
#[cfg(feature = "rated")]
const EXPECTED_RATED_ENTRIES: usize = 527;

/// Lowest and highest assigned id in each view today. Ontology ids start
/// at 1 and are dense so far — nothing has retired yet — but neither
/// property is promised by the format, only observed by these pins. The
/// rated view's range is a subrange: it draws from the same ledger and
/// simply does not carry the ontology-only classes.
#[cfg(feature = "ontology")]
const EXPECTED_ONTOLOGY_ID_RANGE: (u16, u16) = (1, 632);
/// See [`EXPECTED_ONTOLOGY_ID_RANGE`].
#[cfg(feature = "rated")]
const EXPECTED_RATED_ID_RANGE: (u16, u16) = (3, 629);

/// FNV-1a 64 over `(id, mid)` pairs sorted by id.
///
/// Binds each mid to its number, so the hash moves for a renumbering even
/// when the same set of ids and the same set of mids survive it — two
/// classes trading ids is the failure mode that matters most and the one
/// an id-set or mid-set checksum would miss.
///
/// Sorting first makes the hash independent of table order: this pins the
/// *assignment*, not the iteration order of `events()`, which
/// `modules.rs`'s count tests and the assets themselves already cover.
fn fingerprint<S: AsRef<str>>(assignment: &[(u16, S)]) -> u64 {
  const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const PRIME: u64 = 0x0000_0100_0000_01b3;

  let mut sorted: Vec<(u16, &str)> = assignment
    .iter()
    .map(|(id, mid)| (*id, mid.as_ref()))
    .collect();
  sorted.sort_unstable();

  let mut hash = OFFSET;
  let mut eat = |byte: u8| {
    hash ^= u64::from(byte);
    hash = hash.wrapping_mul(PRIME);
  };
  for (id, mid) in &sorted {
    for byte in id.to_le_bytes() {
      eat(byte);
    }
    for &byte in mid.as_bytes() {
      eat(byte);
    }
    // Record separator, so `(1, "ab")` and `(1, "a") (_, "b")` cannot
    // collide by concatenation.
    eat(0xff);
  }
  hash
}

/// Parse a ledger: `#` comment prologue, one header row, then `id,mid`.
///
/// Read through a conforming CSV reader in the same dialect the generator
/// writes, so every mid the writer can emit is accepted — including one
/// that would need quoting. A hand-rolled splitter here would reject valid
/// generator output, turning a legitimate upstream mid into a test
/// failure.
fn parse_ledger(source: &str) -> Vec<(u16, String)> {
  let mut reader = csv::ReaderBuilder::new()
    .has_headers(true)
    .comment(Some(b'#'))
    .from_reader(source.as_bytes());

  let headers: Vec<String> = reader
    .headers()
    .expect("ledger has a header row")
    .iter()
    .map(str::to_string)
    .collect();
  assert_eq!(
    headers,
    ["id", "mid"],
    "unexpected ledger columns; this test reads them positionally",
  );

  reader
    .records()
    .map(|record| {
      let record = record.expect("ledger row parses as CSV");
      let raw = record.get(0).expect("id column");
      let id: u16 = raw
        .parse()
        .unwrap_or_else(|e| panic!("ledger id {raw:?} is not a u16: {e}"));
      (id, record.get(1).expect("mid column").to_string())
    })
    .collect()
}

/// Every row of the committed ledger, live and retired.
fn ledger_rows() -> Vec<(u16, String)> {
  parse_ledger(LEDGER)
}

// ---------------------------------------------------------------------
// The ledger, on its own — no feature needed
// ---------------------------------------------------------------------

/// The ledger is well-formed as a ledger: ascending, gapless in its own
/// ordering guarantee, no duplicate id, no duplicate mid, no reserved 0.
#[test]
fn ledger_is_well_formed() {
  let mut by_id = BTreeMap::<u16, String>::new();
  let mut by_mid = BTreeMap::<String, u16>::new();
  let mut previous = 0u16;
  for (id, mid) in ledger_rows() {
    assert_ne!(id, 0, "ledger assigns the reserved id 0 to {mid:?}");
    assert!(
      id > previous,
      "ledger is not sorted by ascending id: {id} follows {previous}",
    );
    previous = id;
    if let Some(other) = by_id.insert(id, mid.clone()) {
      panic!("ledger assigns id {id} to both {other:?} and {mid:?}");
    }
    if let Some(other) = by_mid.insert(mid.clone(), id) {
      panic!("ledger lists {mid:?} twice, as id {other} and id {id}");
    }
  }
  assert_eq!(by_id.len(), EXPECTED_LEDGER_ROWS);
}

/// Pin every ledger row, tombstones included.
///
/// This is the guard the shipped-table pins cannot be: a retired row is
/// invisible to them, and losing one lets the generator remint its id for
/// a different class with every other check still green. See
/// [`LEDGER_FINGERPRINT`].
#[test]
fn ledger_including_retired_rows_is_pinned() {
  let rows = ledger_rows();

  assert_eq!(
    rows.len(),
    EXPECTED_LEDGER_ROWS,
    "the ledger row count changed. It may only ever GROW: a retirement \
     keeps its row so the id stays burned. If this went down, a row was \
     deleted and its id can now be reminted for a different class.",
  );

  assert_eq!(
    fingerprint(&rows),
    LEDGER_FINGERPRINT,
    "the ledger changed. Read the assets/sound_ids.csv diff: a row that \
     DISAPPEARED is the defect — its id is no longer burned and the next \
     mint can hand it to a different class, breaking every id already \
     stored for the old one. Update this constant only for added rows or \
     for a mid corrected in place with its id kept.",
  );
}

/// The ledger parser must accept everything the generator's writer can
/// emit. No AudioSet mid needs CSV quoting today, but the writer quotes
/// whatever would, and reading it back must recover the original string
/// rather than panicking or splitting it — the mid is the join key that
/// holds an id to its class.
#[test]
fn parser_accepts_mids_that_need_quoting() {
  let source = concat!(
    "# a comment prologue, skipped\n",
    "id,mid\n",
    "1,/m/plain\n",
    "2,\"/m/comma, inside\"\n",
    "3,\"/m/quoted \"\"here\"\"\"\n",
    // A newline inside a quoted field, and a line that then *starts*
    // with the comment marker — the field must survive whole rather
    // than half of it being eaten as a comment.
    "4,\"/m/newline\n# not a comment\"\n",
  );

  let rows = parse_ledger(source);
  assert_eq!(
    rows,
    vec![
      (1, "/m/plain".to_string()),
      (2, "/m/comma, inside".to_string()),
      (3, "/m/quoted \"here\"".to_string()),
      (4, "/m/newline\n# not a comment".to_string()),
    ],
  );
}

// ---------------------------------------------------------------------
// `SoundEventId` itself — no feature needed
// ---------------------------------------------------------------------

/// `new` / `get` are inverse, over the whole `u16` space including the ids
/// the dataset does not carry.
#[test]
fn sound_event_id_wraps_any_u16() {
  for raw in [0, 1, 42, 632, 633, u16::MAX] {
    assert_eq!(SoundEventId::new(raw).get(), raw);
  }
}

/// `Display` is the storage-facing face of the id — a bare number, the
/// same text a database column or a log line would carry.
#[test]
fn sound_event_id_displays_as_its_number() {
  assert_eq!(SoundEventId::new(0).to_string(), "0");
  assert_eq!(SoundEventId::new(632).to_string(), "632");
  assert_eq!(SoundEventId::new(u16::MAX).to_string(), "65535");
}

/// Ordering follows the number, so ids sort and range-scan the way a
/// storage layer expects.
#[test]
fn sound_event_id_orders_by_number() {
  assert!(SoundEventId::new(1) < SoundEventId::new(2));
  assert_eq!(SoundEventId::new(7), SoundEventId::new(7));
  let mut ids = [
    SoundEventId::new(9),
    SoundEventId::new(1),
    SoundEventId::new(5),
  ];
  ids.sort_unstable();
  assert_eq!(
    ids,
    [
      SoundEventId::new(1),
      SoundEventId::new(5),
      SoundEventId::new(9)
    ]
  );
}

/// The id is `#[repr(transparent)]` over a `u16` and must stay that way:
/// downstream stores it as a two-byte column, and the wire face
/// (`serde(transparent)` under the `serde` feature) is the bare number.
#[test]
fn sound_event_id_is_two_bytes() {
  assert_eq!(
    core::mem::size_of::<SoundEventId>(),
    core::mem::size_of::<u16>(),
  );
}

#[cfg(feature = "serde")]
#[test]
fn sound_event_id_serializes_as_a_bare_number() {
  // Not an object, not a tuple — the same text `Display` writes, so a
  // JSON column and a database column carry the same thing.
  let json = serde_json::to_string(&SoundEventId::new(632)).expect("serialize");
  assert_eq!(json, "632");
}

// ---------------------------------------------------------------------
// The `ontology` view
// ---------------------------------------------------------------------

#[cfg(feature = "ontology")]
mod ontology {
  use super::*;
  use soundevents_dataset::ontology::SoundEvent;
  use std::collections::BTreeSet;

  /// The `(id, mid)` assignment as the shipped ontology table carries it.
  pub(super) fn assignment() -> Vec<(u16, &'static str)> {
    SoundEvent::events()
      .iter()
      .map(|e| (e.id().get(), e.mid()))
      .collect()
  }

  /// Every entry resolves back to *itself* through its id.
  ///
  /// Identity is checked by mid, not by pointer: the generated entries are
  /// `const` items, so each reference site — `events()` here, `BY_ID`
  /// inside `from_id` — may be handed its own promoted allocation, and
  /// `std::ptr::eq` would fail on entries that are the same entry. The mid
  /// is unique across the table (the codegen and `ledger_is_well_formed`
  /// both enforce it), so matching mids is identity. A full-value
  /// comparison would be no stronger and would walk the whole child DAG.
  #[test]
  fn from_id_round_trips_every_entry() {
    for entry in SoundEvent::events() {
      let back = SoundEvent::from_id(entry.id()).unwrap_or_else(|| {
        panic!(
          "{:?} carries id {} but from_id returned None",
          entry.mid(),
          entry.id(),
        )
      });
      assert_eq!(
        (back.mid(), back.id()),
        (entry.mid(), entry.id()),
        "id {} resolved to {:?}, not to {:?}",
        entry.id(),
        back.mid(),
        entry.mid(),
      );
    }
  }

  /// Ids are injective — no two entries share one. Without this,
  /// `from_id` could round-trip every entry and still lose a class.
  #[test]
  fn ids_are_unique_across_the_table() {
    let mut by_id = BTreeMap::<u16, &str>::new();
    for entry in SoundEvent::events() {
      if let Some(other) = by_id.insert(entry.id().get(), entry.mid()) {
        panic!(
          "id {} is carried by both {:?} and {:?}",
          entry.id(),
          other,
          entry.mid(),
        );
      }
    }
    assert_eq!(by_id.len(), SoundEvent::events().len());
  }

  /// `from_id` is total over the whole `u16` space: every id either
  /// resolves to the entry that claims it, or resolves to nothing. The
  /// failure this rules out is an unassigned id quietly landing on a
  /// neighbouring class.
  #[test]
  fn from_id_is_total_over_u16() {
    let assigned: BTreeMap<u16, &str> = assignment().into_iter().collect();

    for raw in 0..=u16::MAX {
      match SoundEvent::from_id(SoundEventId::new(raw)) {
        Some(entry) => {
          assert_eq!(
            entry.id().get(),
            raw,
            "from_id({raw}) returned {:?}, which claims id {}",
            entry.mid(),
            entry.id(),
          );
          assert_eq!(
            assigned.get(&raw),
            Some(&entry.mid()),
            "from_id({raw}) resolved to an entry the table does not list under that id",
          );
        }
        None => assert!(
          !assigned.contains_key(&raw),
          "id {raw} is carried by {:?} but from_id returned None",
          assigned[&raw],
        ),
      }
    }
  }

  /// 0 is reserved as "never assigned", so a zeroed storage column fails
  /// to resolve instead of naming whichever entry sits first.
  #[test]
  fn id_zero_is_never_assigned() {
    assert!(SoundEvent::from_id(SoundEventId::new(0)).is_none());
    for entry in SoundEvent::events() {
      assert_ne!(entry.id().get(), 0, "{:?} carries id 0", entry.mid());
    }
  }

  /// Pin the complete ontology assignment. A regeneration that renumbers
  /// anything fails here.
  #[test]
  fn id_assignment_is_pinned() {
    let assignment = assignment();

    assert_eq!(
      assignment.len(),
      EXPECTED_ONTOLOGY_ENTRIES,
      "entry count changed; see `ontology_count_matches_upstream` in \
       tests/modules.rs first",
    );

    let ids: BTreeSet<u16> = assignment.iter().map(|&(id, _)| id).collect();
    let range = (
      *ids.iter().next().expect("non-empty"),
      *ids.iter().next_back().expect("non-empty"),
    );
    assert_eq!(
      range, EXPECTED_ONTOLOGY_ID_RANGE,
      "the live id range moved. A mint legitimately raises the top and a \
       retirement at either end legitimately moves that end — but an \
       existing entry's id changing does not. Check the ledger diff before \
       updating this.",
    );

    assert_eq!(
      fingerprint(&assignment),
      ONTOLOGY_FINGERPRINT,
      "the permanent-id assignment changed. Read the \
       `assets/sound_ids.csv` diff: if an existing id now names a \
       different class, that is the defect — ids are PERMANENT, and every \
       id already stored downstream now points at the wrong entry. Only \
       update this constant once the diff shows nothing but fresh mints, \
       retirements, or upstream mid corrections that kept their ids.",
    );
  }

  /// The shipped table and the committed ledger agree, in both
  /// directions. `generated.rs` is machine-written and unreviewable at
  /// 632 entries; the ledger is the reviewable form of the same
  /// assignment, and this is what keeps the two from drifting apart.
  #[test]
  fn table_and_ledger_agree() {
    let ledger: BTreeMap<u16, String> = ledger_rows().into_iter().collect();

    // Every shipped entry is in the ledger under the same id.
    for entry in SoundEvent::events() {
      let id = entry.id().get();
      assert_eq!(
        ledger.get(&id).map(String::as_str),
        Some(entry.mid()),
        "{:?} ships with id {id}, which the ledger does not assign to it",
        entry.mid(),
      );
    }

    // And every *live* ledger row is in the table. Rows the table does
    // not carry are retirements: legitimate, and their ids must stay
    // unresolvable rather than being handed to someone else.
    let shipped: BTreeMap<u16, &str> = assignment().into_iter().collect();
    for (&id, mid) in &ledger {
      match shipped.get(&id) {
        Some(&live) => assert_eq!(
          live, mid,
          "ledger id {id} names {mid:?} but the table ships {live:?} under it",
        ),
        None => assert!(
          SoundEvent::from_id(SoundEventId::new(id)).is_none(),
          "id {id} ({mid:?}) is retired from the table but still resolves",
        ),
      }
    }
  }

  /// Falsification: [`ONTOLOGY_FINGERPRINT`] must actually be sensitive
  /// to renumbering. Each case below is a way the assignment could break;
  /// all three must move the fingerprint.
  #[test]
  fn renumber_probe_trips_the_pin() {
    let real = assignment();
    assert_eq!(
      fingerprint(&real),
      ONTOLOGY_FINGERPRINT,
      "control: the untouched assignment must match the pin",
    );

    // (a) Two entries trade ids. Same id set, same mid set, same entry
    //     count — nothing but the binding moved, and every id stored for
    //     either class now names the other.
    let mut traded = real.clone();
    let (first, second) = (traded[0].0, traded[1].0);
    traded[0].0 = second;
    traded[1].0 = first;
    assert_ne!(
      fingerprint(&traded),
      ONTOLOGY_FINGERPRINT,
      "two entries traded ids and the pin did not notice",
    );

    // (b) The whole table renumbered — e.g. a generator regressing to
    //     0-based positional ids.
    let shifted: Vec<(u16, &str)> = real.iter().map(|&(id, mid)| (id - 1, mid)).collect();
    assert_ne!(
      fingerprint(&shifted),
      ONTOLOGY_FINGERPRINT,
      "the whole assignment shifted and the pin did not notice",
    );

    // (c) A class retires and its id is handed to a newcomer — exactly
    //     what the "never reused" clause forbids.
    let mut reused = real.clone();
    let (retired_id, _) = reused.remove(0);
    reused.push((retired_id, "/m/a_class_that_did_not_exist_before"));
    assert_ne!(
      fingerprint(&reused),
      ONTOLOGY_FINGERPRINT,
      "a retired id was reused and the pin did not notice",
    );

    // Negative control: reordering the pairs is not a renumbering, and
    // must not trip the pin — otherwise reordering `events()` would be
    // reported as an identity change.
    let mut reordered = real.clone();
    reordered.reverse();
    assert_eq!(
      fingerprint(&reordered),
      ONTOLOGY_FINGERPRINT,
      "the pin is sensitive to table order; it must pin the assignment only",
    );
  }

  /// Falsification for the ledger pin, covering the case the table pins
  /// structurally cannot: a tombstone going missing.
  #[test]
  fn tombstone_loss_trips_the_ledger_pin() {
    let real = ledger_rows();
    assert_eq!(
      fingerprint(&real),
      LEDGER_FINGERPRINT,
      "control: the untouched ledger must match the pin",
    );

    // Drop the highest-id row, as if an editor had tidied away a retired
    // entry. The generator's high-water mark would fall with it and the
    // next new class would be minted that number.
    let mut without_highest = real.clone();
    let highest = without_highest
      .iter()
      .enumerate()
      .max_by_key(|(_, (id, _))| *id)
      .map(|(index, _)| index)
      .expect("ledger is non-empty");
    without_highest.remove(highest);
    assert_ne!(
      fingerprint(&without_highest),
      LEDGER_FINGERPRINT,
      "a ledger row disappeared and the pin did not notice",
    );

    // The same loss is invisible to the ontology pin whenever the lost
    // row is a tombstone — which is exactly why both pins exist. Assert
    // that asymmetry rather than assuming it.
    let live: Vec<(u16, &str)> = real
      .iter()
      .filter(|(id, _)| SoundEvent::from_id(SoundEventId::new(*id)).is_some())
      .map(|(id, mid)| (*id, mid.as_str()))
      .collect();
    assert_eq!(
      fingerprint(&live),
      ONTOLOGY_FINGERPRINT,
      "the live subset of the ledger must reproduce the ontology pin",
    );
  }
}

// ---------------------------------------------------------------------
// The `rated` view
// ---------------------------------------------------------------------

#[cfg(feature = "rated")]
mod rated {
  use super::*;
  use soundevents_dataset::rated::RatedSoundEvent;
  use std::collections::BTreeSet;

  /// The `(id, mid)` assignment as the shipped rated table carries it.
  pub(super) fn assignment() -> Vec<(u16, &'static str)> {
    RatedSoundEvent::events()
      .iter()
      .map(|e| (e.id().get(), e.mid()))
      .collect()
  }

  /// See the `ontology` twin of this test for why identity is checked by
  /// mid rather than by pointer.
  #[test]
  fn from_id_round_trips_every_entry() {
    for entry in RatedSoundEvent::events() {
      let back = RatedSoundEvent::from_id(entry.id()).unwrap_or_else(|| {
        panic!(
          "{:?} carries id {} but from_id returned None",
          entry.mid(),
          entry.id(),
        )
      });
      assert_eq!(
        (back.mid(), back.id(), back.index()),
        (entry.mid(), entry.id(), entry.index()),
        "id {} resolved to {:?}, not to {:?}",
        entry.id(),
        back.mid(),
        entry.mid(),
      );
    }
  }

  #[test]
  fn ids_are_unique_across_the_table() {
    let mut by_id = BTreeMap::<u16, &str>::new();
    for entry in RatedSoundEvent::events() {
      if let Some(other) = by_id.insert(entry.id().get(), entry.mid()) {
        panic!(
          "id {} is carried by both {:?} and {:?}",
          entry.id(),
          other,
          entry.mid(),
        );
      }
    }
    assert_eq!(by_id.len(), RatedSoundEvent::events().len());
  }

  /// Total over `u16` for this view too — including the 105 ids the
  /// ontology carries and this view does not. Those must answer `None`,
  /// not the class sitting in a neighbouring slot: the `BY_ID` table is
  /// dense over the shared id space, so an off-by-one in codegen would
  /// show up exactly here.
  #[test]
  fn from_id_is_total_over_u16() {
    let assigned: BTreeMap<u16, &str> = assignment().into_iter().collect();

    for raw in 0..=u16::MAX {
      match RatedSoundEvent::from_id(SoundEventId::new(raw)) {
        Some(entry) => {
          assert_eq!(
            entry.id().get(),
            raw,
            "from_id({raw}) returned {:?}, which claims id {}",
            entry.mid(),
            entry.id(),
          );
          assert_eq!(
            assigned.get(&raw),
            Some(&entry.mid()),
            "from_id({raw}) resolved to an entry the table does not list under that id",
          );
        }
        None => assert!(
          !assigned.contains_key(&raw),
          "id {raw} is carried by {:?} but from_id returned None",
          assigned[&raw],
        ),
      }
    }
  }

  #[test]
  fn id_zero_is_never_assigned() {
    assert!(RatedSoundEvent::from_id(SoundEventId::new(0)).is_none());
    for entry in RatedSoundEvent::events() {
      assert_ne!(entry.id().get(), 0, "{:?} carries id 0", entry.mid());
    }
  }

  /// Pin the complete rated assignment. See
  /// [`ONTOLOGY_FINGERPRINT`](super::ONTOLOGY_FINGERPRINT) for the rules
  /// on updating one of these.
  #[test]
  fn id_assignment_is_pinned() {
    let assignment = assignment();

    assert_eq!(
      assignment.len(),
      EXPECTED_RATED_ENTRIES,
      "entry count changed; see `rated_count_matches_csv` in \
       tests/modules.rs first",
    );

    let ids: BTreeSet<u16> = assignment.iter().map(|&(id, _)| id).collect();
    let range = (
      *ids.iter().next().expect("non-empty"),
      *ids.iter().next_back().expect("non-empty"),
    );
    assert_eq!(
      range, EXPECTED_RATED_ID_RANGE,
      "the rated view's id range moved. It is a subrange of the ontology's, \
       so it also moves when the ontology-only class at either end changes — \
       but an existing entry's id changing does not. Check the ledger diff \
       before updating this.",
    );

    assert_eq!(
      fingerprint(&assignment),
      RATED_FINGERPRINT,
      "the rated view's permanent-id assignment changed. Read the \
       `assets/sound_ids.csv` diff: if an existing id now names a \
       different class, that is the defect — ids are PERMANENT.",
    );
  }

  /// Every rated entry is in the ledger under the same id, and every id
  /// the ledger holds that this view does not carry stays unresolvable
  /// here rather than being handed to a neighbour.
  #[test]
  fn table_and_ledger_agree() {
    let ledger: BTreeMap<u16, String> = ledger_rows().into_iter().collect();

    for entry in RatedSoundEvent::events() {
      let id = entry.id().get();
      assert_eq!(
        ledger.get(&id).map(String::as_str),
        Some(entry.mid()),
        "{:?} ships with id {id}, which the ledger does not assign to it",
        entry.mid(),
      );
    }

    let shipped: BTreeMap<u16, &str> = assignment().into_iter().collect();
    for (&id, mid) in &ledger {
      match shipped.get(&id) {
        Some(&live) => assert_eq!(
          live, mid,
          "ledger id {id} names {mid:?} but the table ships {live:?} under it",
        ),
        None => assert!(
          RatedSoundEvent::from_id(SoundEventId::new(id)).is_none(),
          "id {id} ({mid:?}) is not in the rated view but still resolves there",
        ),
      }
    }
  }

  /// The model output index is deliberately *not* the id: it is scoped to
  /// a released model's label ordering and moves on a retrain, which is
  /// the whole reason a separate id exists. Assert they are genuinely
  /// different numbers rather than trusting the prose — an id that
  /// happened to equal `index + 1` everywhere would be a codegen bug
  /// wearing the right shape.
  #[test]
  fn the_id_is_not_the_model_output_index() {
    let differs = RatedSoundEvent::events()
      .iter()
      .filter(|e| usize::from(e.id().get()) != e.index() + 1)
      .count();
    assert!(
      differs > 0,
      "every rated id is exactly `index + 1`; the id has collapsed onto \
       the model output index and no longer survives a retrain",
    );
  }
}

// ---------------------------------------------------------------------
// The two views against each other
// ---------------------------------------------------------------------

/// One ledger, two views: a class present in both carries the same id in
/// each, and the rated ids are a strict subset of the ontology's.
///
/// This is the property that would be lost to a per-view ledger, and it is
/// what lets a downstream store hold one `SoundEventId` column regardless
/// of which view produced the row.
#[cfg(all(feature = "ontology", feature = "rated"))]
#[test]
fn rated_ids_agree_with_the_ontology() {
  use soundevents_dataset::{ontology::SoundEvent, rated::RatedSoundEvent};
  use std::collections::BTreeSet;

  let ontology: BTreeMap<&str, u16> = ontology::assignment()
    .into_iter()
    .map(|(id, mid)| (mid, id))
    .collect();

  for entry in RatedSoundEvent::events() {
    assert_eq!(
      ontology.get(entry.mid()).copied(),
      Some(entry.id().get()),
      "{:?} carries id {} in the rated view and a different one in the \
       ontology",
      entry.mid(),
      entry.id(),
    );

    // And the id resolves to the same class through both views.
    assert_eq!(
      SoundEvent::from_id(entry.id()).map(SoundEvent::mid),
      Some(entry.mid()),
    );
  }

  // The 105 ontology-only classes hold ids the rated view must refuse.
  let rated_ids: BTreeSet<u16> = rated::assignment().into_iter().map(|(id, _)| id).collect();
  let ontology_only = ontology
    .values()
    .filter(|&&id| !rated_ids.contains(&id))
    .count();
  assert_eq!(
    ontology_only,
    EXPECTED_ONTOLOGY_ENTRIES - EXPECTED_RATED_ENTRIES,
    "the ontology-only class count changed",
  );
  for (mid, &id) in &ontology {
    if !rated_ids.contains(&id) {
      assert!(
        RatedSoundEvent::from_id(SoundEventId::new(id)).is_none(),
        "id {id} ({mid:?}) is ontology-only but resolves in the rated view",
      );
    }
  }
}
