#[allow(dead_code)]
#[path = "../src/backend/mod.rs"]
mod backend;

use backend::{CosmeticsConfig, Loader, Profile};
use std::collections::HashMap;

#[test]
fn new_cosmetics_survive_round_trip_and_legacy_settings_migrate() {
    let mut c = CosmeticsConfig::default();
    c.sky.meteor_size = 3.;
    c.sky.bodies.push(backend::SkyBody {
        name: "Sun".into(),
        color: "#FFCC55".into(),
        azimuth: -45.,
        elevation: 30.,
        size: 12.,
        enabled: true,
    });
    c.ui.background = "#223344".into();
    let mut json = serde_json::to_value(&c).unwrap();
    let d: CosmeticsConfig = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(d.sky.bodies.len(), 2);
    assert_eq!(d.sky.meteor_size, 3.);
    assert_eq!(d.ui.background, "#223344");
    json["sky"].as_object_mut().unwrap().remove("meteorSize");
    json["sky"].as_object_mut().unwrap().remove("bodies");
    json["ui"].as_object_mut().unwrap().remove("background");
    let old: CosmeticsConfig = serde_json::from_value(json).unwrap();
    assert_eq!(old.sky.meteor_size, 1.);
    assert_eq!(old.sky.bodies.len(), 1);
    assert!(backend::storage::validate_cosmetics(&old).is_ok());
    c.sky.bodies[0].size = f32::INFINITY;
    assert!(backend::storage::validate_cosmetics(&c).is_err());
}

fn profile() -> Profile {
    Profile {
        id: "qa-1_21".into(),
        name: "QA world".into(),
        version: "1.21.1".into(),
        loader: Loader::Fabric,
        loader_version: None,
        icon: "sparkles".into(),
        color: "#B8C9FF".into(),
        memory_mb: 4096,
        java_path: String::new(),
        last_played: None,
        mods: vec![],
        cosmetics: CosmeticsConfig::default(),
    }
}

#[test]
fn instance_identifiers_cannot_escape_data_directory() {
    for id in [
        "../outside",
        "..\\outside",
        "C:\\Windows",
        "/tmp",
        "",
        ".",
        "..",
        "profile:stream",
        "a/b",
    ] {
        assert!(
            backend::storage::validate_id(id).is_err(),
            "accepted {id:?}"
        );
    }
    assert!(backend::storage::validate_id("qa-1_21").is_ok());
}

#[test]
fn mod_filenames_reject_paths_and_non_jars() {
    for file in [
        "../bad.jar",
        "..\\bad.jar",
        "C:\\bad.jar",
        "bad.jar:payload",
        "bad.exe",
        "mod.jar/other",
        "mod.jar\\other",
    ] {
        assert!(
            backend::storage::valid_mod_file(file).is_err(),
            "accepted {file:?}"
        );
    }
    for file in ["aster-cosmetics-0.1.0.jar", "fabric-api.jar.disabled"] {
        assert!(
            backend::storage::valid_mod_file(file).is_ok(),
            "rejected {file:?}"
        );
    }
}

#[test]
fn catalog_range_keeps_releases_and_excludes_snapshots() {
    for version in ["1.7", "1.7.10", "1.8.9", "1.21.1", "26.1", "26.3"] {
        assert!(
            backend::storage::supported_version(version),
            "rejected {version}"
        );
    }
    for version in [
        "1.6.4",
        "26.4",
        "27.1",
        "26.3-snapshot-1",
        "1.21.1-pre1",
        "../1.21.1",
        "",
    ] {
        assert!(
            !backend::storage::supported_version(version),
            "accepted {version}"
        );
    }
}

#[test]
fn cosmetics_bridge_uses_expected_camel_case_schema() {
    let mut original = CosmeticsConfig::default();
    original.hud.fps.x = 0.73;
    original.sky.meteor_frequency = 0.0;
    original.ui.reduced_motion = true;
    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["ui"]["reducedMotion"], true);
    assert_eq!(json["sky"]["meteorFrequency"], 0.0);
    assert!(json["sky"].get("meteor_frequency").is_none());
    let restored: CosmeticsConfig = serde_json::from_value(json).unwrap();
    assert!((restored.hud.fps.x - 0.73).abs() < f32::EPSILON);
    assert!(backend::storage::validate_cosmetics(&restored).is_ok());
}

#[test]
fn unsafe_customization_never_reaches_java_bridge() {
    let mut config = CosmeticsConfig::default();
    config.hud.fps.x = f32::NAN;
    assert!(backend::storage::validate_cosmetics(&config).is_err());
    config.hud.fps.x = 1.1;
    assert!(backend::storage::validate_cosmetics(&config).is_err());
    config = CosmeticsConfig::default();
    config.sky.color = "red; anything".into();
    assert!(backend::storage::validate_cosmetics(&config).is_err());
}

#[test]
fn invalid_profile_cannot_change_instance_paths() {
    let mut p = profile();
    assert!(backend::storage::validate_profile(&p).is_ok());
    p.id = "../../existing-minecraft".into();
    assert!(backend::storage::validate_profile(&p).is_err());
    p = profile();
    p.memory_mb = u32::MAX;
    assert!(backend::storage::validate_profile(&p).is_err());
}

#[test]
fn substitutions_preserve_spaces_as_part_of_one_argument() {
    let vars = HashMap::from([
        (
            "game_directory".into(),
            "C:\\Users\\A player\\Minecraft".into(),
        ),
        ("auth_player_name".into(), "QA_Player".into()),
    ]);
    assert_eq!(
        backend::install::substitute("${game_directory}", &vars),
        "C:\\Users\\A player\\Minecraft"
    );
    assert_eq!(
        backend::install::substitute("name=${auth_player_name}", &vars),
        "name=QA_Player"
    );
}

#[test]
fn old_launch_arguments_keep_quotes_and_literal_shell_characters() {
    let args = backend::install::split_legacy_arguments(
        "--username Player --gameDir \"C:/A player/game\" --literal a&b",
    )
    .unwrap();
    assert_eq!(
        args,
        vec![
            "--username",
            "Player",
            "--gameDir",
            "C:/A player/game",
            "--literal",
            "a&b"
        ]
    );
    assert!(backend::install::split_legacy_arguments("--gameDir \"unterminated").is_err());
}

#[test]
fn rule_order_and_host_os_control_downloads() {
    assert!(backend::install::rules_allow(&serde_json::json!({})));
    assert!(!backend::install::rules_allow(
        &serde_json::json!({"rules":[{"action":"allow","os":{"name":"linux"}}]})
    ));
    assert!(backend::install::rules_allow(
        &serde_json::json!({"rules":[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]})
    ));
    assert!(!backend::install::rules_allow(
        &serde_json::json!({"rules":[{"action":"allow"},{"action":"disallow"}]})
    ));
}

#[test]
fn downloaded_paths_remain_inside_the_game_directory() {
    let root = std::env::temp_dir().join("aster-path-test");
    for path in [
        "../outside.jar",
        "libraries/../../outside.jar",
        "C:/Windows/file",
        "//server/share",
        "libraries\\file.jar",
        "/absolute.jar",
    ] {
        assert!(
            backend::install::safe_join(&root, path).is_err(),
            "accepted {path}"
        );
    }
    assert_eq!(
        backend::install::safe_join(&root, "org/example/mod.jar").unwrap(),
        root.join("org/example/mod.jar")
    );
}

#[test]
fn profile_customization_survives_a_real_disk_round_trip() {
    let root = std::env::temp_dir().join(format!("aster-qa-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut store = backend::storage::Store::load(root.clone()).unwrap();
        let mut p = profile();
        p.name = "  Custom valley  ".into();
        p.cosmetics.sky.preset = "aurora".into();
        p.cosmetics.hud.fps.x = 0.68;
        store.save_profile(p).unwrap();
        let instance = store.instance("qa-1_21").unwrap();
        assert!(instance.starts_with(&root));
        drop(store);
        let restored = backend::storage::Store::load(root.clone()).unwrap();
        let p = restored.profile("qa-1_21").unwrap();
        assert_eq!(p.name, "Custom valley");
        assert_eq!(p.cosmetics.sky.preset, "aurora");
        assert!((p.cosmetics.hud.fps.x - 0.68).abs() < f32::EPSILON);
        assert!(std::fs::read_to_string(root.join("state.json"))
            .unwrap()
            .contains("Custom valley"));
    })();
    if let (Ok(resolved), Ok(temp_root)) =
        (root.canonicalize(), std::env::temp_dir().canonicalize())
    {
        assert!(resolved.starts_with(&temp_root) && resolved != temp_root);
        assert!(resolved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aster-qa-"));
        let _ = std::fs::remove_dir_all(resolved);
    }
    result
}

#[test]
fn java_requirement_follows_mojang_metadata_and_legacy_boundaries() {
    for (version, expected) in [
        ("1.7.10", 8),
        ("1.16.5", 8),
        ("1.17.1", 16),
        ("1.20.4", 17),
        ("1.20.5", 21),
    ] {
        assert_eq!(
            backend::install::required_java(version, &serde_json::json!({})).unwrap(),
            expected
        );
    }
    assert_eq!(
        backend::install::required_java(
            "26.3",
            &serde_json::json!({"javaVersion":{"majorVersion":25}})
        )
        .unwrap(),
        25
    );
    assert!(backend::install::required_java("26.3", &serde_json::json!({})).is_err());
}

#[test]
fn java_archive_rejects_traversal_before_writing_outside_runtime() {
    use std::io::Write;
    let root = std::env::temp_dir().join(format!("aster-qa-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let archive = root.join("unsafe.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
    writer
        .start_file("../outside.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(b"never write this outside the runtime")
        .unwrap();
    writer.finish().unwrap();
    let destination = root.join("runtime");
    std::fs::create_dir_all(&destination).unwrap();
    assert!(backend::install::extract_runtime_zip(&archive, &destination).is_err());
    assert!(!root.join("outside.txt").exists());
    let resolved = root.canonicalize().unwrap();
    let temp_root = std::env::temp_dir().canonicalize().unwrap();
    assert!(resolved.starts_with(&temp_root) && resolved != temp_root);
    assert!(resolved
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("aster-qa-"));
    std::fs::remove_dir_all(resolved).unwrap();
}

#[test]
#[ignore = "Downloads an official Temurin runtime; run explicitly when verifying provisioning"]
fn provision_managed_java_25_from_official_source() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-results/managed-java");
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let client = reqwest::blocking::Client::builder()
        .user_agent("AsterLauncher/0.1.0 QA")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    let (events, receiver) = std::sync::mpsc::channel();
    let monitor = std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if let backend::BackendEvent::Progress(progress) = event {
                println!("{}", progress.message);
            }
        }
    });
    let runtime =
        backend::install::provision_java(&root, &client, 25, &events, "qa-managed-java").unwrap();
    assert_eq!(runtime.major, 25);
    assert!(std::path::Path::new(&runtime.path).starts_with(root.join("runtimes")));
    assert_eq!(
        backend::install::inspect_java(&runtime.path).unwrap().major,
        25
    );
    println!(
        "Verified real app-local Java {} at {}",
        runtime.version, runtime.path
    );
    drop(events);
    monitor.join().unwrap();
}

#[test]
#[ignore = "Fetches Mojang metadata and Fabric API; run explicitly to verify mod installation"]
fn install_embedded_cosmetics_and_compatible_fabric_api() {
    let project = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = project.join("test-results/cosmetics-install");
    let (commands, events) = backend::Backend::spawn(root.clone());
    commands
        .send(backend::BackendCommand::InstallCosmetics(
            "aster-default".into(),
        ))
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    let mut installed = false;
    while std::time::Instant::now() < deadline {
        match events.recv_timeout(std::time::Duration::from_secs(15)) {
            Ok(backend::BackendEvent::Progress(progress)) => {
                println!("{}", progress.message);
                if progress.message.contains("Aster Cosmetics installed") {
                    installed = true;
                    break;
                }
            }
            Ok(backend::BackendEvent::Error(error)) => {
                panic!("Cosmetics installation failed: {error}")
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                println!("Waiting for official mod metadata…")
            }
            Err(error) => panic!("Backend disconnected: {error}"),
        }
    }
    assert!(installed, "Cosmetics install did not finish before timeout");
    let mods = root.join("instances/aster-default/mods");
    let jar = mods.join("aster-cosmetics-0.1.0.jar");
    assert_eq!(
        std::fs::read(&jar).unwrap(),
        std::fs::read(project.join("cosmetics-mod/build/libs/aster-cosmetics-0.1.0.jar")).unwrap()
    );
    let archive = zip::ZipArchive::new(std::fs::File::open(jar).unwrap()).unwrap();
    assert!(
        archive.file_names().any(|name| name.ends_with(".dll")),
        "Bundled cosmetics JAR is missing its Rust native library"
    );
    assert!(std::fs::read_dir(&mods)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("fabric-api-")));
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("instances/aster-default/config/aster-cosmetics.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(config["schemaVersion"], 1);
    println!("Verified embedded Rust cosmetic JAR, matching Fabric API, and per-instance config");
}
