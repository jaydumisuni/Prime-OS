#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <cstdio>
static const wchar_t* kKey=L"Software\\Classes\\CLSID\\{5D5B74A4-FA57-4F52-9826-5918955DF021}\\InprocServer32";
int wmain() {
    wchar_t module[MAX_PATH]; DWORD n=GetModuleFileNameW(nullptr,module,MAX_PATH); if(!n || n>=MAX_PATH) return 2;
    wchar_t* slash=wcsrchr(module,L'\\'); if(!slash) return 3;
    wcscpy_s(slash+1, MAX_PATH-(slash+1-module), L"custom-com-x64.dll");
    HKEY key=nullptr; LONG r=RegCreateKeyExW(HKEY_LOCAL_MACHINE,kKey,0,nullptr,0,KEY_SET_VALUE,nullptr,&key,nullptr);
    if(r!=ERROR_SUCCESS) return 4;
    r=RegSetValueExW(key,nullptr,0,REG_SZ,(const BYTE*)module,(DWORD)((wcslen(module)+1)*sizeof(wchar_t)));
    const wchar_t model[]=L"Apartment";
    if(r==ERROR_SUCCESS) r=RegSetValueExW(key,L"ThreadingModel",0,REG_SZ,(const BYTE*)model,sizeof(model));
    RegCloseKey(key); if(r!=ERROR_SUCCESS) return 5;
    std::printf("PRIME_W5_REGISTER_OK\n"); return 0;
}
