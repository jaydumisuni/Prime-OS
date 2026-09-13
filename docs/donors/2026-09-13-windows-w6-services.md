# W6 Windows services donor record

Prime W6 reuses Wine 11.0 Service Control Manager behavior from the sealed W5 predecessor environment. The sealed image exposes `services.exe` and both x64/x86 `sc.exe` lanes under the Wine WoW64 layout.

A KRATOS feasibility proof used hash-bound native service/controller fixtures through `/usr/libexec/prime/prime-windows-provider-w1`. Both x64 and WoW64 created `PrimeW6Proof`, reached `SERVICE_RUNNING`, accepted STOP, reached STOPPED, deleted the service, and returned `PRIME_W6_SERVICE_OK` with exit 0. This is donor feasibility evidence; final promotion still requires isolation, recovery, adversarial lifecycle, exact frozen-SHA replay, Sergeant, Formula, and remote binding.
