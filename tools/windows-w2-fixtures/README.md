# Prime Windows W2 proof fixtures

These source-only fixtures prove the W2 installer lifecycle without shipping third-party installers or proof binaries in the repository.

- `w2-exe-installer.c` builds a deterministic PE32+ installer that writes `C:\\PrimeW2\\exe-installed.txt`.
- `w2-msi-installer.wxs` builds an MSI that installs `C:\\PrimeW2\\msi-installed.txt`.
- `w2-slow-installer.c` records a mutation window, sleeps for three seconds, then writes the final verified payload for interruption/concurrency proof.
- `normalize_msi.py` validates and normalizes only MSI SummaryInformation FILETIMEs and the single embedded CAB file timestamp so independent `wixl` builds are byte-identical.
- `*-manifest.json` are exact sealed W2 component manifests for the proof-only `/proof/` mount.

Proof toolchain: Fedora 44, `mingw64-gcc-16.1.1-1.fc44`, `msitools-0.106.58-1.fc44`, with `libfaketime-0.9.12-12.fc44` available for metadata investigation. The executable compiler uses `-Os -s -Wl,--no-insert-timestamp`.

Current deterministic artifact identities established on KRATOS:

- EXE: `sha256:5591777f3543d0d3f6e9c70e38f5c7030b076a5f923a3f15cd5ebbb3e7b5291c`
- MSI: `sha256:c8d1e2c3e272a8e917d307bd30edaa6d07358cc386fb3f3c8d7c9052c53118df`
- slow EXE: `sha256:a06746ebab03af5676fe9d351e99d34c2e31666c8eba4f40660cf59a19b3d183`

The proof binaries remain under the KRATOS cockpit and are not committed.
