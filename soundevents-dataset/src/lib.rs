#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "ontology")]
#[cfg_attr(docsrs, doc(cfg(feature = "ontology")))]
pub mod ontology;

#[cfg(feature = "rated")]
#[cfg_attr(docsrs, doc(cfg(feature = "rated")))]
pub mod rated;

#[cfg(feature = "ontology")]
#[cfg_attr(docsrs, doc(cfg(feature = "ontology")))]
pub use ontology::SoundEvent;

#[cfg(feature = "rated")]
#[cfg_attr(docsrs, doc(cfg(feature = "rated")))]
pub use rated::RatedSoundEvent;

/// A permanent, stable handle on one sound event in the dataset.
///
/// Store this — not a display name, not an AudioSet mid, not a model
/// output index — when something outside the crate needs to refer to a
/// class across time: a database column, a search index, a wire message.
/// `SoundEvent::from_id` / `RatedSoundEvent::from_id` resolve it back.
///
/// # The guarantee
///
/// An id is assigned once and never changes; a class whose display name,
/// description or restrictions are corrected upstream keeps its id, a
/// class dropped from the ontology never has its id handed out again, and
/// a new class mints a fresh one. The assignment lives in
/// `assets/sound_ids.csv` and is pinned by the crate's tests: a
/// regeneration that renumbers anything fails them loudly.
///
/// Ids start at 1. `SoundEventId::new(0)` is well-formed but is never an
/// assigned id, so a zeroed column always fails to resolve rather than
/// silently naming the first entry.
///
/// # One id space, two views
///
/// The ids span the full AudioSet ontology, and the `rated` view is a
/// subset of it keyed on the same mids — so a class present in both
/// carries one and the same id in each. An id the `rated` view does not
/// carry resolves through `RatedSoundEvent::from_id` to `None` while
/// still resolving through `SoundEvent::from_id`.
///
/// # Not the model output index
///
/// `RatedSoundEvent::index` is a *position* in a released model's output
/// vector. It is scoped to that model's label ordering and moves whenever
/// upstream retrains or re-releases; a `SoundEventId` does not. Index into
/// the model's output with the index, store the id.
///
/// # Not a validated type
///
/// Constructing a `SoundEventId` asserts nothing — any `u16` makes one,
/// and unassigned and retired ids are both representable. The `from_id`
/// lookups are total and decide: they answer `None` for every id the view
/// does not carry.
///
/// ```
/// use soundevents_dataset::SoundEventId;
///
/// let stored: u16 = SoundEventId::new(42).get();
/// assert_eq!(SoundEventId::new(stored).to_string(), "42");
///
/// // Ordered by the number, so ids sort and range-scan the way a storage
/// // layer expects.
/// assert!(SoundEventId::new(1) < SoundEventId::new(2));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct SoundEventId(u16);

impl SoundEventId {
  /// Wrap a raw id — typically one read back out of storage.
  ///
  /// Performs no validation; see the [type docs](Self) for why, and the
  /// `from_id` lookups for the ones that do decide.
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn new(id: u16) -> Self {
    Self(id)
  }

  /// The raw id, for handing to storage.
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn get(&self) -> u16 {
    self.0
  }
}

impl core::fmt::Display for SoundEventId {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    core::fmt::Display::fmt(&self.0, f)
  }
}

/// Errors that can occur when parsing a [`Restriction`] from a string.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("unknown restriction: {0}")]
pub struct UnknownRestriction<'a>(&'a str);

impl<'a> UnknownRestriction<'a> {
  /// Get the name associated with the `UnknownRestriction` error
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn name(&self) -> &'a str {
    self.0
  }
}

impl<'a> TryFrom<&'a str> for Restriction {
  type Error = UnknownRestriction<'a>;

  #[cfg_attr(not(tarpaulin), inline(always))]
  fn try_from(value: &'a str) -> Result<Self, Self::Error> {
    Ok(match value {
      "abstract" | "ABSTRACT" | "Abstract" => Restriction::Abstract,
      "blacklist" | "BLACKLIST" | "BlackList" | "blackList" | "Blacklist" => Restriction::Blacklist,
      _ => return Err(UnknownRestriction(value)),
    })
  }
}

/// A restriction on a sound entry, which may be an abstract category or a blacklisted entry
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum Restriction {
  /// For a class that is principally a container within the hierarchy, but will not have any explicit examples for itself. "Human voice" is an abstract class. Abstract classes will always have children.
  Abstract,
  /// For classes that have been excluded from rating for the time being. These are classes that we found were too difficult for raters to mark reliably, or for which we had too much trouble finding candidates, or which we decided to drop from labeling for some other reason.
  Blacklist,
}

impl core::fmt::Display for Restriction {
  #[cfg_attr(not(tarpaulin), inline(always))]
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

impl Restriction {
  /// Get the string representation of the restriction
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn as_str(&self) -> &'static str {
    match self {
      Self::Abstract => "abstract",
      Self::Blacklist => "blacklist",
    }
  }

  /// Return `true` if the restriction is an abstract category
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn is_abstract(&self) -> bool {
    matches!(self, Self::Abstract)
  }

  /// Return `true` if the restriction is a blacklisted entry.
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn is_blacklist(&self) -> bool {
    matches!(self, Self::Blacklist)
  }
}

/// Defines a sound-event struct (with all its accessors), its companion
/// `Unknown*Code` error type, and `Display`. Used twice — once in the
/// `ontology` module and once in the `rated` module — to produce two
/// independent types with identical shape.
#[cfg(any(feature = "ontology", feature = "rated"))]
macro_rules! define_sound_event {
  (
    $(#[$struct_meta:meta])*
    name: $name:ident,
    $(#[$err_meta:meta])*
    error: $err_name:ident,
    error_message: $err_msg:literal
    $(,
      extra_fields: {
        $($extra_field:tt)*
      }
    )?
    $(,
      extra_impl: {
        $($extra_impl:tt)*
      }
    )?
    $(,)?
  ) => {
    $(#[$struct_meta])*
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize))]
    pub struct $name {
      pub(crate) id: $crate::SoundEventId,
      #[cfg_attr(feature = "serde", serde(skip))]
      pub(crate) code: i64,
      pub(crate) mid: &'static str,
      pub(crate) name: &'static str,
      #[cfg_attr(feature = "serde", serde(skip_serializing_if = "<[_]>::is_empty"))]
      pub(crate) aliases: &'static [&'static str],
      pub(crate) description: &'static str,
      #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
      pub(crate) citation_uri: ::core::option::Option<&'static str>,
      #[cfg_attr(feature = "serde", serde(skip_serializing_if = "<[_]>::is_empty"))]
      pub(crate) children: &'static [&'static $name],
      #[cfg_attr(feature = "serde", serde(skip_serializing_if = "<[_]>::is_empty"))]
      pub(crate) restrictions: &'static [$crate::Restriction],
      $($($extra_field)*)?
    }

    impl ::core::fmt::Display for $name {
      #[cfg_attr(not(tarpaulin), inline(always))]
      fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(self.name)
      }
    }

    impl $name {
      /// This entry's permanent [`SoundEventId`](crate::SoundEventId) — the
      /// handle to store when something outside the crate needs to name it
      /// later.
      ///
      /// Round-trips through [`Self::from_id`] for every entry in this view.
      /// See the [type docs](crate::SoundEventId) for what the permanence
      /// guarantee does and does not cover.
      ///
      /// Prefer this over [`Self::encode`] for anything persisted: the code
      /// is *derived* from the mid, so it cannot survive an upstream
      /// re-midding, while an id can be held to its class by editing one
      /// ledger row.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn id(&self) -> $crate::SoundEventId {
        self.id
      }

      /// The entry carrying `id` in this view, or `None` if it has no such
      /// id.
      ///
      /// Total over every `u16`: unassigned ids, ids retired by a past
      /// dataset revision, the reserved 0, and — in the `rated` view — ids
      /// held by classes outside it all answer `None` rather than resolving
      /// to some neighbouring entry.
      ///
      /// This is the reverse of [`Self::id`], and the pair is a bijection
      /// onto the ids this view carries: `from_id(e.id())` is `e` for every
      /// `e` in [`Self::events`], and an id that resolves always resolves to
      /// the entry that claims it.
      ///
      /// O(1): a bounds check and one load through a dense id → entry table
      /// generated alongside the entries themselves.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn from_id(id: $crate::SoundEventId) -> ::core::option::Option<&'static Self> {
        let slot = id.get() as ::core::primitive::usize;
        if slot < generated::BY_ID.len() {
          generated::BY_ID[slot]
        } else {
          ::core::option::Option::None
        }
      }

      /// Get the unique code for the sound entry, which is a hash of its mid.
      ///
      /// The mid is the entry's stable upstream identity, so the code
      /// survives an upstream edit to the display name. The hash is 32 bits
      /// wide and is widened to a signed `i64` so it can be stored directly
      /// in databases whose native integer width is signed; every code is
      /// therefore non-negative and no larger than `u32::MAX`. Codes are
      /// unique within a module — the code generator refuses to emit a table
      /// with a collision. The value is an opaque identifier: compare it for
      /// equality, do not order it or do arithmetic on it.
      ///
      /// [`Self::id`] is the better handle for new storage: it is half the
      /// width, orders meaningfully, and is *assigned* rather than derived,
      /// so it can outlive a change to the mid it was first minted for.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn encode(&self) -> i64 {
        self.code
      }

      /// Get the sound entry's AudioSet machine id, such as `"/m/09x0r"`.
      ///
      /// This is upstream's identifier for the class and the join key this
      /// crate's codegen matches on. It is provenance, not a storage handle:
      /// store [`Self::id`] instead.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn mid(&self) -> &'static str {
        self.mid
      }

      /// Get the sound entry's name
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn name(&self) -> &'static str {
        self.name
      }

      /// Get the sound entry's description
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn description(&self) -> &'static str {
        self.description
      }

      /// Get the sound entry's aliases
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn aliases(&self) -> &'static [&'static str] {
        self.aliases
      }

      /// Get the sound entry's citation url, if any
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn citation_uri(&self) -> ::core::option::Option<&'static str> {
        self.citation_uri
      }

      /// Get the sound entry's children sound entries
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn children(&self) -> &'static [&'static Self] {
        self.children
      }

      /// Get the sound entry's restrictions
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn restrictions(&self) -> &'static [$crate::Restriction] {
        self.restrictions
      }

      /// Return `true` if the sound entry carries an ontology
      /// [`Restriction::Blacklist`](crate::Restriction) restriction.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn is_blacklisted(&self) -> bool {
        let mut i = 0;
        while i < self.restrictions.len() {
          if matches!(self.restrictions[i], $crate::Restriction::Blacklist) {
            return true;
          }
          i += 1;
        }
        false
      }

      $($($extra_impl)*)?
    }

    $(#[$err_meta])*
    #[derive(Debug, ::thiserror::Error, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[error($err_msg)]
    pub struct $err_name(pub(crate) i64);

    impl $err_name {
      /// Get the code associated with this error.
      #[cfg_attr(not(tarpaulin), inline(always))]
      pub const fn code(&self) -> i64 {
        self.0
      }
    }
  };
}

#[cfg(any(feature = "ontology", feature = "rated"))]
pub(crate) use define_sound_event;
