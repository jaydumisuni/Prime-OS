//! Explicit, compile-time isolated MOB-1 Linux guest **unqualified** generation.
//!
//! Not bootc, not secure image enrollment, and never eligible for physical
//! acceptance. This provides read-only Core transport testing in a RAM-only
//! QEMU guest without forging bootc status or changing Desktop production.
use prime_contracts::{GenerationRecord, GenerationState, ReleaseChannel, GENERATION_SCHEMA};
use std::io;
use std::path::Path;

const LAB_STATE: &str = "/run/prime-mob1-lab";
const EVIDENCE: &str = "prime.mob1.generic-guest.unqualified.v0";

fn canonical_sha256(s: &str) -> bool {
    s.len() == 71 && s.starts_with("sha256:") &&
        s[7..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Retains a real CI-computed input digest but deliberately makes no claim
/// that the guest attested its own bootchain or that bootc is installed.
/// 
/// Only available when the crate was built with `mob1-guest-lab`; the normal
/// `primed` binary still requires actual bootc status/generation binding.
pub fn unqualified_generation(state_dir: &Path, input_digest: &str,
                              observed_at: &str) -> io::Result<GenerationRecord> {
    if std::env::consts::ARCH != "aarch64" || state_dir != Path::new(LAB_STATE) ||
        !canonical_sha256(input_digest) || observed_at.is_empty() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "MOB1_LAB_MODE_NOT_ALLOWED_OR_INPUT_UNQUALIFIED"));
    }
    Ok(GenerationRecord {
        schema: GENERATION_SCHEMA.to_owned(),
        generation_id: format!("mob1-unqualified-{}", &input_digest[7..19]),
        image_digest: input_digest.to_owned(),
        channel: ReleaseChannel::Lab,
        created_at: observed_at.to_owned(),
        source_revision: "mob1-generic-guest-unqualified".to_owned(),
        state: GenerationState::BootTry,
        boot_attempts_remaining: Some(1),
        evidence_refs: vec![EVIDENCE.to_owned()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_outside_canonical_arm64_ephemeral_path() {
        let ok = format!("sha256:{}", "a".repeat(64));
        assert!(unqualified_generation(Path::new("/var/lib/prime"), &ok, "now").is_err());
        assert!(unqualified_generation(Path::new("/run/prime-mob1-lab"), "sha256:garbage", "now").is_err());
    }
    #[test]
    fn never_generates_known_good_or_bootc_proof() {
        let digest = format!("sha256:{}", "b".repeat(64));
        if std::env::consts::ARCH != "aarch64" {
            assert!(unqualified_generation(Path::new(LAB_STATE), &digest, "now").is_err());
        } else {
            let record=unqualified_generation(Path::new(LAB_STATE), &digest, "now").expect("lab record");
            assert_eq!(record.channel, ReleaseChannel::Lab);
            assert_eq!(record.state, GenerationState::BootTry);
            assert!(!record.evidence_refs.iter().any(|r| r.contains("bootc")));
        }
    }
}
