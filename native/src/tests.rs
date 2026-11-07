use crate::{
    model::{self, Contact, Log},
    rig,
    storage::Storage,
};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

#[test]
fn imports_both_legacy_formats() {
    for data in [
        r#"[{"callsign":"DL0AH","frequency":7.191,"id":42}]"#,
        r#"{"contacts":[{"callsign":"DL0AH","frequency":7.191,"id":42}]}"#,
    ] {
        let log = Log::parse(data.as_bytes()).unwrap();
        assert_eq!(log.contacts[0].id, "42");
        assert_eq!(log.contacts[0].frequency, "7.191");
        assert_eq!(log.contacts[0].kind, "Other");
    }
}
#[test]
fn rejects_bad_import_without_silent_loss() {
    for data in [
        r#"{"contacts":[null]}"#,
        r#"{"version":3,"contacts":[]}"#,
        r#"{"contacts":[{"id":"same"},{"id":"same"}]}"#,
        r#"{"contacts":[{"notes":{}}]}"#,
        r#"{"contacts":[{"datetime":"not a date"}]}"#,
    ] {
        assert!(Log::parse(data.as_bytes()).is_err(), "{data}");
    }
}
#[test]
fn roundtrip_preserves_fields_and_unknown_metadata() {
    let source=br#"{"version":2,"types":[{"name":"Custom","color":"rose"}],"contacts":[{"id":"x","type":"Custom","datetime":"2026-11-07T19:00","callsign":"DL0AH","satellite":"ISS","frequency":"145.8","mode":"FM","rstSent":"59","rstRcvd":"57","location":"Wales","recording":"recording.wav","notes":"test","futureField":42}]}"#;
    let log = Log::parse(source).unwrap();
    let bytes = serde_json::to_vec(&log).unwrap();
    assert_eq!(Log::parse(&bytes).unwrap(), log);
}
#[test]
fn merge_keeps_existing_ids_and_unifies_type_case() {
    let a=Log::parse(br#"{"types":[{"name":"Ham","color":"sky"}],"contacts":[{"id":"a","callsign":"OLD","type":"Ham"}]}"#).unwrap();
    let b=Log::parse(br#"{"types":[{"name":"ham","color":"rose"}],"contacts":[{"id":"a","callsign":"NEW","type":"ham"},{"id":"b","type":"ham"}]}"#).unwrap();
    let c = a.merge(b);
    assert_eq!(c.contacts.len(), 2);
    assert_eq!(c.contacts[0].callsign, "OLD");
    assert_eq!(c.types.len(), 1);
    assert_eq!(c.contacts[1].kind, "Ham");
}
#[test]
fn rename_and_delete_types_cascade() {
    let mut log = Log::default();
    log.contacts.push(Contact::fresh("Ham".into()));
    log.rename_type("Ham", "Amateur").unwrap();
    assert_eq!(log.contacts[0].kind, "Amateur");
    assert!(log.rename_type("Amateur", "Satellite").is_err());
    let fallback = log.delete_type("Amateur").unwrap();
    assert_eq!(log.contacts[0].kind, fallback);
}
#[test]
fn exact_filters_do_not_match_notes_or_suffixes() {
    let c = Contact {
        callsign: " dl0ah ".into(),
        frequency: "145.800".into(),
        location: " Cardiff ".into(),
        notes: "OTHER".into(),
        ..Default::default()
    };
    assert!(c.matches("Callsign", "DL0AH"));
    assert!(!c.matches("Callsign", "OTHER"));
    assert!(!c.matches("Callsign", "DL0AH/P"));
    assert!(c.matches("Frequency", "145.8"));
    assert!(c.matches("Location", "cardiff"));
}
#[test]
fn utc_offset_is_converted_and_bad_dates_rejected() {
    assert_eq!(
        model::canonical_date("2026-07-01T20:00:00+01:00").unwrap(),
        "2026-07-01T19:00"
    );
    assert!(model::canonical_date("2026-02-30T12:00").is_err());
}
#[test]
fn classify_radio_types() {
    for (f, m, t) in [
        (7.191, "LSB", "Ham"),
        (137.9, "WFM", "Satellite"),
        (1698.0, "HRPT", "Satellite"),
        (105.7, "WFM", "Other"),
        (5.35, "USB", "Other"),
        (145.5, "FM", "Ham"),
    ] {
        assert_eq!(model::infer_type(f, m), t);
    }
}
#[test]
fn saves_and_backs_up_exact_previous_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::open(dir.path().into()).unwrap();
    let first = Log::default();
    store.save(&first).unwrap();
    let bytes = fs::read(store.path()).unwrap();
    let mut next = first.clone();
    next.contacts.push(Contact::fresh("Ham".into()));
    store.save(&next).unwrap();
    assert_eq!(store.load().unwrap(), next);
    let backup = fs::read_dir(dir.path().join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read(backup).unwrap(), bytes);
}
#[test]
fn backup_failure_leaves_live_file_intact() {
    let dir = tempfile::tempdir().unwrap();
    let store = Storage::open(dir.path().into()).unwrap();
    store.save(&Log::default()).unwrap();
    let before = fs::read(store.path()).unwrap();
    fs::write(dir.path().join("backups"), b"blocking file").unwrap();
    assert!(store.save(&Log::default()).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), before);
}
#[test]
fn corrupt_saved_log_is_not_replaced_on_load() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("radio-log.json"), b"broken").unwrap();
    let store = Storage::open(dir.path().into()).unwrap();
    assert!(store.load().is_err());
    assert_eq!(fs::read(store.path()).unwrap(), b"broken");
}
#[test]
fn concurrent_writer_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let first = Storage::open(dir.path().into()).unwrap();
    assert!(Storage::open(dir.path().into()).is_err());
    drop(first);
    assert!(Storage::open(dir.path().into()).is_ok());
}
fn mock_reply(chunks: Vec<&'static [u8]>) -> Result<rig::Radio, String> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut command = [0; 4];
        stream.read_exact(&mut command).unwrap();
        assert_eq!(&command, b"f\nm\n");
        for chunk in chunks {
            if stream.write_all(chunk).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
    let result = rig::read_radio(addr);
    server.join().unwrap();
    result
}
#[test]
fn rig_waits_for_complete_fragmented_reply() {
    let r = mock_reply(vec![b"145800000\nFM\n12", b"500\n"]).unwrap();
    assert_eq!(r.mhz, 145.8);
    assert_eq!(r.mode, "FM");
}
#[test]
fn rig_rejects_errors_disconnect_and_invalid_values() {
    for chunks in [
        vec![b"RPRT -1\n".as_slice()],
        vec![b"145800000\n".as_slice()],
        vec![b"NaN\nFM\n12500\n".as_slice()],
    ] {
        assert!(mock_reply(chunks).is_err());
    }
}
