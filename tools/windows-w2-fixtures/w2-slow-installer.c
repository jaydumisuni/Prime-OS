#include <windows.h>
#include <stdio.h>

static unsigned long long now_filetime(void) {
    FILETIME ft;
    ULARGE_INTEGER u;
    GetSystemTimeAsFileTime(&ft);
    u.LowPart = ft.dwLowDateTime;
    u.HighPart = ft.dwHighDateTime;
    return u.QuadPart;
}

int WINAPI WinMain(HINSTANCE instance, HINSTANCE previous, LPSTR command_line, int show) {
    (void)instance; (void)previous; (void)command_line; (void)show;
    CreateDirectoryA("C:\\PrimeW2", NULL);
    HANDLE log = CreateFileA("C:\\PrimeW2\\mutation-window.txt", GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (log == INVALID_HANDLE_VALUE) return 31;
    char line[96]; DWORD written = 0;
    int n = snprintf(line, sizeof(line), "START=%llu\r\n", now_filetime());
    if (n <= 0 || !WriteFile(log, line, (DWORD)n, &written, NULL) || written != (DWORD)n) { CloseHandle(log); return 32; }
    FlushFileBuffers(log);
    Sleep(3000);
    n = snprintf(line, sizeof(line), "END=%llu\r\n", now_filetime());
    if (n <= 0 || !WriteFile(log, line, (DWORD)n, &written, NULL) || written != (DWORD)n) { CloseHandle(log); return 33; }
    CloseHandle(log);
    const char payload[] = "PRIME_W2_SLOW_INSTALLED\r\n";
    HANDLE file = CreateFileA("C:\\PrimeW2\\slow-installed.txt", GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return 34;
    BOOL ok = WriteFile(file, payload, (DWORD)(sizeof(payload) - 1), &written, NULL);
    CloseHandle(file);
    return ok && written == sizeof(payload) - 1 ? 0 : 35;
}
