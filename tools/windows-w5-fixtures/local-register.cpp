#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstdio>
#include <cwchar>
static const wchar_t* kKey=L"Software\\Classes\\CLSID\\{4B733C73-893F-4A4F-93F4-E8E8A101A5B2}\\LocalServer32";
int wmain(){
    wchar_t module[MAX_PATH]; DWORD n=GetModuleFileNameW(nullptr,module,MAX_PATH); if(!n||n>=MAX_PATH)return 2;
    wchar_t* slash=wcsrchr(module,L'\\'); if(!slash)return 3;
#ifdef _WIN64
    const wchar_t* server=L"local-server-x64.exe";
#else
    const wchar_t* server=L"local-server-x86.exe";
#endif
    wcscpy_s(slash+1,MAX_PATH-(slash+1-module),server);
    HKEY key=nullptr; LONG rr=RegCreateKeyExW(HKEY_LOCAL_MACHINE,kKey,0,nullptr,0,KEY_SET_VALUE,nullptr,&key,nullptr);if(rr!=ERROR_SUCCESS)return 4;
    rr=RegSetValueExW(key,nullptr,0,REG_SZ,(const BYTE*)module,(DWORD)((wcslen(module)+1)*sizeof(wchar_t)));RegCloseKey(key);if(rr!=ERROR_SUCCESS)return 5;
    std::printf("PRIME_W5_LOCAL_REGISTER_OK\n");return 0;
}
