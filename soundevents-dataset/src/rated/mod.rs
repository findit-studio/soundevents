//! The released AudioSet rated label set — the 527 classes from
//! [`class_labels_indices.csv`](https://research.google.com/audioset/download.html)
//! that AudioSet annotators actually labeled on YouTube clips.
//!
//! This is a strict subset of the [`ontology`](crate::ontology) module:
//! abstract container nodes are excluded. Blacklisted classes are not — 12
//! of the 527 released classes still carry an ontology
//! [`crate::Restriction::Blacklist`] restriction. Upstream published them
//! in `class_labels_indices.csv` regardless, and a model's output still
//! carries a score at their [`RatedSoundEvent::index`] slot, so they stay
//! in this table too. Callers that want to filter them out by policy
//! should check [`RatedSoundEvent::restrictions`] (or the
//! [`RatedSoundEvent::is_blacklisted`] shorthand) themselves. The children
//! of a [`RatedSoundEvent`] reference only other rated entries, so
//! traversing the hierarchy stays inside the rated namespace.

crate::define_sound_event! {
  /// A sound entry in the rated AudioSet label set.
  name: RatedSoundEvent,
  /// Errors that can occur when looking up a [`RatedSoundEvent`] by its code.
  error: UnknownRatedSoundEventCode,
  error_message: "unknown rated sound event code: {0}",
  extra_fields: {
    pub(crate) index: usize,
  },
  extra_impl: {
    /// Get the model output index for this rated entry.
    #[cfg_attr(not(tarpaulin), inline(always))]
    pub const fn index(&self) -> usize {
      self.index
    }
  },
}

mod generated;
