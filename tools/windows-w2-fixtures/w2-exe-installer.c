#include <windows.h>

int WINAPI WinMain(HINSTANCE instance, HINSTANCE previous, LPSTR command_line, int show) {
    (void)instance; (void)previous; (void)command_line; (void)show;
    const char *directory = "C:\\PrimeW2";
    const char *path = "C:\\PrimeW2\\exe-installed.txt";
    const char payload[] = "PRIME_W2_EXE_INSTALLED\r\n";
    CreateDirectoryA(directory, NULL);
    HANDLE file = CreateFileA(path, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return 21;
    DWORD written = 0;
    BOOL ok = WriteFile(file, payload, (DWORD)(sizeof(payload) - 1), &written, NULL);
    CloseHandle(file);
    return ok && written == sizeof(payload) - 1 ? 0 : 22;
}
