#include <windows.h>
#include <setupapi.h>
#include <cfgmgr32.h>
#include <algorithm>
#include <cctype>
#include <cstdio>
#include <string>

#ifndef PRIME_W7_DEFAULT_VID
#define PRIME_W7_DEFAULT_VID "19D2"
#endif
#ifndef PRIME_W7_DEFAULT_PID
#define PRIME_W7_DEFAULT_PID "0581"
#endif

static std::string upper(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char ch) { return (char)std::toupper(ch); });
    return value;
}

int main(int argc, char **argv) {
    if (argc != 1 && argc != 3) {
        std::fprintf(stderr, "usage: device-enum [VID PID]\n");
        return 64;
    }
    const std::string vid = "VID_" + upper(argc == 3 ? argv[1] : PRIME_W7_DEFAULT_VID);
    const std::string pid = "PID_" + upper(argc == 3 ? argv[2] : PRIME_W7_DEFAULT_PID);

    HDEVINFO set = SetupDiGetClassDevsA(nullptr, "USB", nullptr, DIGCF_PRESENT | DIGCF_ALLCLASSES);
    if (set == INVALID_HANDLE_VALUE) {
        std::fprintf(stderr, "SetupDiGetClassDevs failed %lu\n", GetLastError());
        return 65;
    }

    for (DWORD index = 0;; ++index) {
        SP_DEVINFO_DATA dev{};
        dev.cbSize = sizeof(dev);
        if (!SetupDiEnumDeviceInfo(set, index, &dev)) {
            if (GetLastError() == ERROR_NO_MORE_ITEMS) break;
            std::fprintf(stderr, "SetupDiEnumDeviceInfo failed %lu\n", GetLastError());
            SetupDiDestroyDeviceInfoList(set);
            return 66;
        }

        char instance_id[MAX_DEVICE_ID_LEN]{};
        if (!SetupDiGetDeviceInstanceIdA(set, &dev, instance_id, sizeof(instance_id), nullptr)) continue;
        const std::string id = upper(instance_id);
        if (id.find(vid) == std::string::npos || id.find(pid) == std::string::npos) continue;

        SetupDiDestroyDeviceInfoList(set);
        std::printf("PRIME_W7_USB_ENUM_OK %s %s INSTANCE=%s\n", vid.c_str(), pid.c_str(), instance_id);
        return 0;
    }

    SetupDiDestroyDeviceInfoList(set);
    std::printf("PRIME_W7_USB_ABSENT %s %s\n", vid.c_str(), pid.c_str());
    return 3;
}
