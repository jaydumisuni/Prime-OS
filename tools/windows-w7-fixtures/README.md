# W7 USB proof fixture

`device-enum.cpp` is a read-only Windows SetupAPI proof fixture. It enumerates present USB devnodes, matches an explicit VID/PID, and reports only the matching Windows instance ID. It performs no device I/O, control transfers, resets, interface claims, writes, or firmware operations.

Build the fixtures with the pinned MinGW toolchain using fully static C++ runtimes so the sealed Prime Wine image does not depend on host-side MinGW DLLs:

```text
x86_64-w64-mingw32-g++ -O2 -static device-enum.cpp -lsetupapi -lcfgmgr32 -o device-enum-x64.exe
i686-w64-mingw32-g++ -O2 -static device-enum.cpp -lsetupapi -lcfgmgr32 -o device-enum-x86.exe
```

The fixture is intentionally enumeration-only. Device-node authorization is proven separately by Prime's systemd policy compiler (`DevicePolicy=closed` plus the exact `/dev/bus/usb/BBB/DDD` `DeviceAllow`) so the Windows proof cannot mutate the attached device.
