# W5 COM donor record

Prime W5 reuses Wine 11.0 COM rather than introducing a second COM implementation. The exact W4 sealed proof image `localhost/prime-w4-rootfs:proof-f99c7f8` already exposes x64/x86 `ole32.dll`, `oleaut32.dll`, `combase.dll`, `rpcrt4.dll`, `rpcss.exe`, `regsvr32.exe`, `wscript.exe`, `cscript.exe`, and `scrrun.dll`.

A direct KRATOS spike on the sealed W4 image proved `CreateObject("Scripting.Dictionary")`, `Add`, and `Item` through x64 `cscript.exe` with exit 0 and marker `PRIME_W5_COM_OK`. The WoW64 `C:\windows\syswow64\cscript.exe` lane produced the same marker and exit 0. Setting `WINEARCH=win32` on the new WoW64 runtime correctly fails because Fedora Wine 11 uses WoW64 mode; W5 therefore treats x86 as the SysWOW64 lane inside the application-scoped prefix, consistent with W4.

These observations are feasibility evidence only. Promotion requires the W5 benchmark, exact-SHA runtime fixtures, lifecycle/adversarial proof, independent review, Formula, and remote binding.
