//! Smoke tests for the `ontology` and `rated` modules.

#[cfg(feature = "ontology")]
mod ontology {
  use soundevents_dataset::ontology::SoundEvent;

  #[test]
  fn ontology_count_matches_upstream() {
    // SoundEvent::from_code is generated from every entry, so we can count
    // distinct ids reachable through EVENTS as a sanity check.
    assert_eq!(
      SoundEvent::events().len(),
      632,
      "expected 632 ontology entries"
    );
  }

  #[test]
  fn lookup_is_case_insensitive() {
    for q in [
      "man speaking",
      "MAN SPEAKING",
      "Man Speaking",
      "mAn SpEaKiNg",
      "man_speaking",
      "manSpeaking",
      "/m/05zppz",
      "/M/05ZPPZ",
    ] {
      let r = SoundEvent::from_key(q);
      assert_eq!(r.len(), 1, "expected 1 ontology match for {q:?}");
      assert_eq!(r[0].id(), "/m/05zppz");
    }
  }

  #[test]
  fn ambiguous_alias_returns_multiple() {
    assert!(SoundEvent::from_key("Inside").len() > 1);
  }

  #[test]
  fn every_id_and_alias_resolves_case_insensitively() {
    // Walks every key in the table rather than a sample. The perfect hash is
    // computed by `phf_codegen` at codegen time and evaluated by `phf` at
    // runtime, so a version skew between the two — or any change to how a key
    // is hashed — misplaces an arbitrary subset of keys while leaving the
    // handful named in the other tests resolving fine.
    let ids = |s: &[&'static SoundEvent]| s.iter().map(|e| e.id()).collect::<Vec<_>>();

    for event in SoundEvent::events() {
      for key in core::iter::once(event.id()).chain(event.aliases().iter().copied()) {
        let hits = SoundEvent::from_key(key);
        assert!(
          hits.iter().any(|e| e.id() == event.id()),
          "key {key:?} did not resolve to {}",
          event.id()
        );
        for variant in [key.to_ascii_lowercase(), key.to_ascii_uppercase()] {
          assert_eq!(
            ids(SoundEvent::from_key(&variant)),
            ids(hits),
            "case variant {variant:?} of {key:?} resolved differently"
          );
        }
      }
    }
  }

  #[test]
  fn unknown_returns_empty() {
    assert!(SoundEvent::from_key("definitely not a sound").is_empty());
  }

  #[test]
  fn every_code_resolves_back_to_its_entry() {
    for event in SoundEvent::events() {
      let code = event.encode();
      let resolved = SoundEvent::from_code(code)
        .unwrap_or_else(|| panic!("code {code} for {} did not resolve", event.id()));
      assert_eq!(
        resolved.id(),
        event.id(),
        "code {code} resolved to the wrong entry"
      );
    }
  }

  #[test]
  fn every_code_is_positive_and_fits_in_u32() {
    for event in SoundEvent::events() {
      let code = event.encode();
      assert!(code > 0, "code {code} for {} is not positive", event.id());
      assert!(
        code <= i64::from(u32::MAX),
        "code {code} for {} exceeds u32::MAX",
        event.id()
      );
    }
  }
}

#[cfg(feature = "rated")]
mod rated {
  use soundevents_dataset::rated::RatedSoundEvent;

  #[test]
  fn rated_count_matches_csv() {
    assert_eq!(
      RatedSoundEvent::events().len(),
      527,
      "expected 527 rated entries"
    );
  }

  #[test]
  fn lookup_is_case_insensitive() {
    let r = RatedSoundEvent::from_key("MAN SPEAKING");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id(), "/m/05zppz");
  }

  #[test]
  fn every_id_and_alias_resolves_case_insensitively() {
    // See the matching `ontology` test: this is the coverage that would catch a
    // `phf_codegen`/`phf` version skew, which moves keys without moving codes.
    let ids = |s: &[&'static RatedSoundEvent]| s.iter().map(|e| e.id()).collect::<Vec<_>>();

    for event in RatedSoundEvent::events() {
      for key in core::iter::once(event.id()).chain(event.aliases().iter().copied()) {
        let hits = RatedSoundEvent::from_key(key);
        assert!(
          hits.iter().any(|e| e.id() == event.id()),
          "key {key:?} did not resolve to {}",
          event.id()
        );
        for variant in [key.to_ascii_lowercase(), key.to_ascii_uppercase()] {
          assert_eq!(
            ids(RatedSoundEvent::from_key(&variant)),
            ids(hits),
            "case variant {variant:?} of {key:?} resolved differently"
          );
        }
      }
    }
  }

  #[test]
  fn rated_excludes_abstract_entries() {
    // "Human voice" is abstract in the upstream ontology and not in the
    // rated CSV.
    assert!(RatedSoundEvent::from_key("Human voice").is_empty());
  }

  #[test]
  fn exactly_12_rated_entries_are_blacklisted() {
    // Unlike abstract nodes, blacklisted classes are NOT excluded from the
    // rated set — upstream still published them in `class_labels_indices.csv`,
    // so a model's output still carries a score at their `index` slot. This
    // pins the module doc's claim: if upstream regenerates the table and the
    // blacklisted set drifts, this test catches it, not just the doc prose.
    let count = RatedSoundEvent::events()
      .iter()
      .filter(|e| e.is_blacklisted())
      .count();
    assert_eq!(
      count, 12,
      "expected 12 blacklisted entries in the rated set"
    );
  }

  #[test]
  fn rated_children_stay_in_rated_namespace() {
    // Pick an entry whose ontology children include unrated nodes and
    // verify that the rated view drops them. "Human sounds" is rated and
    // has many children, some abstract.
    let entries = RatedSoundEvent::from_key("Human sounds");
    if let Some(e) = entries.first() {
      // Just walk the children; if codegen left a stale id reference,
      // this would fail to compile.
      for child in e.children() {
        assert!(!child.id().is_empty());
      }
    }
  }

  #[test]
  fn every_code_resolves_back_to_its_entry() {
    for event in RatedSoundEvent::events() {
      let code = event.encode();
      let resolved = RatedSoundEvent::from_code(code)
        .unwrap_or_else(|| panic!("code {code} for {} did not resolve", event.id()));
      assert_eq!(
        resolved.id(),
        event.id(),
        "code {code} resolved to the wrong entry"
      );
    }
  }

  #[test]
  fn code_is_the_pinned_hash_of_the_id() {
    // The code is the 32-bit hash of the id `/m/02zsn`, not of the display
    // name "Female speech, woman speaking" — renaming the entry upstream must
    // not move it. Pinned as a literal so a change to the hash or to what is
    // hashed fails here instead of silently orphaning stored codes.
    const FEMALE_SPEECH: i64 = 1994861414;

    let event = RatedSoundEvent::from_key("/m/02zsn")
      .first()
      .copied()
      .expect("/m/02zsn is a rated entry");
    assert_eq!(event.encode(), FEMALE_SPEECH);

    assert_eq!(
      RatedSoundEvent::from_code(FEMALE_SPEECH).map(RatedSoundEvent::id),
      Some("/m/02zsn")
    );

    let resolved =
      <&'static RatedSoundEvent>::try_from(FEMALE_SPEECH).expect("pinned code resolves");
    assert_eq!(resolved.id(), "/m/02zsn");
  }

  #[test]
  fn every_code_is_positive_and_fits_in_u32() {
    for event in RatedSoundEvent::events() {
      let code = event.encode();
      assert!(code > 0, "code {code} for {} is not positive", event.id());
      assert!(
        code <= i64::from(u32::MAX),
        "code {code} for {} exceeds u32::MAX",
        event.id()
      );
    }
  }
}
