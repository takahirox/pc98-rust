use pc98_rust::{Machine, Profile};

#[test]
fn unchanged_adv98_starts_pmd_game_music_from_compiled_original_score() {
    let mut m = Machine::new(Profile::Pc9821ApDos);
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/dos-apps/music.hdm"
    ))
    .expect("run python3 tools/prepare_music_fixture.py");
    m.insert(0, bytes, "music.hdm").unwrap();
    m.reset();
    for _ in 0..180 {
        m.step(100_000);
        m.bus.audio.clear();
        assert!(m.error.is_none(), "{:?}", m.error);
    }
    let before = m.bus.opn.key_edges;
    assert_eq!(
        m.bus.opn.sample(),
        0.0,
        "PMD must wait for the game's music-start command"
    );
    m.bus.key(0x1c, true);
    m.step(100_000);
    m.bus.key(0x1c, false);
    let mut energy = 0.0;
    let mut pitches = std::collections::BTreeSet::new();
    for _ in 0..180 {
        m.step(100_000);
        assert!(m.error.is_none(), "{:?}", m.error);
        energy += m
            .bus
            .audio
            .iter()
            .map(|&s| f64::from(s).powi(2))
            .sum::<f64>();
        m.bus.audio.clear();
        pitches.insert(m.bus.opn.registers[0xa0]);
    }
    assert!(
        m.bus.opn.key_edges > before + 16,
        "key edges {} -> {}",
        before,
        m.bus.opn.key_edges
    );
    assert!(pitches.len() >= 3, "sequenced notes {pitches:?}");
    assert!(energy > 10.0, "music energy {energy}");
    assert_eq!(m.bus.beep_edges, 0);
    assert!(m.bus.opn.writes > 1000);
}
