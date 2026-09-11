# Prime Windows Personality W3 proof fixtures

Source-only deterministic fixtures for W3 managed-runtime proof. Generated PE files are proof artifacts and are never committed.

- `w3-console.cs`: console fixture; expected stdout `PRIME_W3_MANAGED_EXECUTED`. Build `/platform:x64` and `/platform:x86`.
- `w3-winforms.cs`: font-neutral WinForms window for visible W3 GUI/Wayland proof. It self-closes after eight seconds so lifecycle proof completes without synthetic input. It intentionally avoids text controls because Windows-font substitution is outside the W3 managed-runtime claim.
