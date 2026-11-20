//! Optional, development-only PHI ARM64 kernel observation command.
//!
//! Reuses prime_hardware::arm64::observe; never enrolls a Host identity,
//! changes machine policy, or claims that an OEM phone is supported.
use prime_hardware::arm64::{candidate_json, observe};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE: &str = "Usage:\n  prime-phi-arm64-observe --live\n  prime-phi-arm64-observe --fixture-root <directory> --arch <aarch64|arm64|x86_64>\n\nThis development-only command reads kernel-projected paths and prints JSON to stdout.\nIt never grants flash/device support or Host identity enrollment.\n";

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Help,
    Live,
    Fixture { root: PathBuf, claimed_arch: String },
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Mode, &'static str> {
    let mut live = false;
    let mut root: Option<PathBuf> = None;
    let mut arch: Option<String> = None;
    let mut help = false;
    let mut args = args.into_iter();
    while let Some(token) = args.next() {
        match token.to_str() {
            Some("--help" | "-h") => help = true,
            Some("--live") if !live => live = true,
            Some("--fixture-root") if root.is_none() => {
                root = Some(PathBuf::from(args.next().ok_or("MISSING_FIXTURE_ROOT")?));
            }
            Some("--arch") if arch.is_none() => {
                let raw = args.next().ok_or("MISSING_ARCH")?;
                let value = raw.into_string().map_err(|_| "INVALID_ARCH")?;
                if !matches!(value.as_str(), "aarch64" | "arm64" | "x86_64") {
                    return Err("UNSUPPORTED_FIXTURE_ARCH");
                }
                arch = Some(value);
            }
            _ => return Err("UNKNOWN_OR_DUPLICATE_ARGUMENT"),
        }
    }
    if help {
        // --help must not turn an otherwise action-eligible invocation into
        // an ambiguous collection mode.
        return if !live && root.is_none() && arch.is_none() {
            Ok(Mode::Help)
        } else {
            Err("HELP_WITH_COLLECTION_ARGUMENTS")
        };
    }
    match (live, root, arch) {
        (true, None, None) => Ok(Mode::Live),
        (false, Some(root), Some(claimed_arch)) => Ok(Mode::Fixture { root, claimed_arch }),
        _ => Err("SPECIFY_EXACTLY_ONE_COLLECTION_MODE"),
    }
}

fn fixture_root(root: &Path) -> Result<PathBuf, &'static str> {
    let resolved = root.canonicalize().map_err(|_| "FIXTURE_ROOT_UNAVAILABLE")?;
    if !resolved.is_dir() || resolved == Path::new("/") {
        return Err("FIXTURE_ROOT_MUST_BE_NON_SYSTEM_DIRECTORY");
    }
    Ok(resolved)
}

fn build_output(mode: Mode) -> Result<serde_json::Value, &'static str> {
    let (root, arch, kind, clock) = match mode {
        Mode::Help => return Err("HELP_NOT_AN_OBSERVATION"),
        Mode::Live => {
            let now = SystemTime::now().duration_since(UNIX_EPOCH)
                .ok().and_then(|d| u64::try_from(d.as_millis()).ok());
            (PathBuf::from("/"), std::env::consts::ARCH.to_string(),
             "LIVE_KERNEL_READ_ONLY", now)
        }
        Mode::Fixture { root, claimed_arch } =>
            (fixture_root(&root)?, claimed_arch, "SYNTHETIC_FIXTURE", None),
    };
    let mut value = candidate_json(&observe(&root, &arch), None);
    let object = value.as_object_mut().ok_or("CANDIDATE_PROJECTION_NOT_OBJECT")?;
    object.insert("capture_mode".to_owned(), serde_json::json!(kind));
    object.insert("host_clock_unix_ms_unattested".to_owned(), serde_json::json!(clock));
    object.insert("collector_version".to_owned(), serde_json::json!("prime-phi-arm64-collector.candidate.v0"));
    object.insert("live_device_qualification".to_owned(), serde_json::json!(false));
    Ok(value)
}

fn main() {
    let mode = match parse_args(std::env::args_os().skip(1)) {
        Ok(mode) => mode,
        Err(err) => {
            eprintln!("{err}\n{USAGE}");
            std::process::exit(64);
        }
    };
    if mode == Mode::Help {
        print!("{USAGE}");
        return;
    }
    let value = match build_output(mode) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(65);
        }
    };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if serde_json::to_writer_pretty(&mut out, &value).is_err() || writeln!(out).is_err() {
        eprintln!("OUTPUT_WRITE_FAILED");
        std::process::exit(74);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Mode, &'static str> {
        parse_args(args.iter().map(|x| OsString::from(*x)))
    }

    #[test]
    fn collection_requires_exactly_one_mode() {
        assert_eq!(p(&[]), Err("SPECIFY_EXACTLY_ONE_COLLECTION_MODE"));
        assert_eq!(p(&["--live"]), Ok(Mode::Live));
        assert!(p(&["--live", "--arch", "aarch64"]).is_err());
        assert!(p(&["--live", "--fixture-root", "/tmp"]).is_err());
    }

    #[test]
    fn fixture_mode_keeps_claimed_arch_separate() {
        assert_eq!(p(&["--fixture-root", "fixtures/arm64", "--arch", "aarch64"]),
            Ok(Mode::Fixture { root: PathBuf::from("fixtures/arm64"), claimed_arch: "aarch64".to_owned() }));
        assert_eq!(p(&["--fixture-root", "fixtures/arm64", "--arch", "x86_64"]),
            Ok(Mode::Fixture { root: PathBuf::from("fixtures/arm64"), claimed_arch: "x86_64".to_owned() }));
    }

    #[test]
    fn duplicate_and_unknown_flags_fail_closed() {
        assert!(p(&["--live", "--live"]).is_err());
        assert!(p(&["--shell", "id"]).is_err());
        assert!(p(&["--arch", "arm64", "--arch", "arm64"]).is_err());
        assert!(p(&["--fixture-root", "dir"]).is_err());
    }

    #[test]
    fn help_does_not_enable_collection() {
        assert_eq!(p(&["--help"]), Ok(Mode::Help));
        assert!(p(&["--help", "--live"]).is_err());
    }

    #[test]
    fn fixture_root_cannot_be_live_system_root() {
        assert_eq!(fixture_root(Path::new("/")),
            Err("FIXTURE_ROOT_MUST_BE_NON_SYSTEM_DIRECTORY"));
        let temp = tempfile::tempdir().expect("temporary fixture");
        assert_eq!(fixture_root(temp.path()), Ok(temp.path().canonicalize().unwrap()));
    }

    #[test]
    fn fixture_json_never_promotes_hardware() {
        let temp = tempfile::tempdir().expect("temporary fixture");
        let value = build_output(Mode::Fixture { root: temp.path().into(), claimed_arch: "aarch64".into() })
            .expect("candidate JSON");
        assert_eq!(value["capture_mode"], "SYNTHETIC_FIXTURE");
        assert!(value["host_clock_unix_ms_unattested"].is_null());
        assert_eq!(value["any_device_functionality_proven"], false);
        assert_eq!(value["firmware_or_device_modified"], false);
        assert_eq!(value["live_device_qualification"], false);
    }
}
