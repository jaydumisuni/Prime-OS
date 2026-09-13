#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winsvc.h>
#include <cstdio>
#include <cwchar>
static bool wait_state(SC_HANDLE s,DWORD want){ for(int i=0;i<100;i++){ SERVICE_STATUS_PROCESS p{}; DWORD n=0; if(!QueryServiceStatusEx(s,SC_STATUS_PROCESS_INFO,(LPBYTE)&p,sizeof(p),&n)) return false; if(p.dwCurrentState==want) return true; Sleep(100); } return false; }
int wmain(){ wchar_t path[MAX_PATH]; DWORD n=GetModuleFileNameW(nullptr,path,MAX_PATH); if(!n||n>=MAX_PATH)return 2; wchar_t* slash=wcsrchr(path,L'\\'); if(!slash)return 3;
#ifdef _WIN64
 const wchar_t* exe=L"service-x64.exe";
#else
 const wchar_t* exe=L"service-x86.exe";
#endif
 wcscpy_s(slash+1,MAX_PATH-(slash+1-path),exe);
 SC_HANDLE scm=OpenSCManagerW(nullptr,nullptr,SC_MANAGER_ALL_ACCESS); if(!scm){std::printf("SCM_OPEN_FAIL %lu\n",GetLastError());return 10;}
 DeleteService(OpenServiceW(scm,L"PrimeW6Proof",DELETE));
 SC_HANDLE s=CreateServiceW(scm,L"PrimeW6Proof",L"Prime W6 Proof",SERVICE_ALL_ACCESS,SERVICE_WIN32_OWN_PROCESS,SERVICE_DEMAND_START,SERVICE_ERROR_NORMAL,path,nullptr,nullptr,nullptr,nullptr,nullptr);
 if(!s){std::printf("CREATE_FAIL %lu\n",GetLastError());CloseServiceHandle(scm);return 11;}
 if(!StartServiceW(s,0,nullptr)){std::printf("START_FAIL %lu\n",GetLastError());DeleteService(s);CloseServiceHandle(s);CloseServiceHandle(scm);return 12;}
 if(!wait_state(s,SERVICE_RUNNING)){std::printf("RUNNING_FAIL\n");return 13;}
 std::printf("PRIME_W6_SERVICE_RUNNING\n"); SERVICE_STATUS st{}; if(!ControlService(s,SERVICE_CONTROL_STOP,&st)){std::printf("STOP_FAIL %lu\n",GetLastError());return 14;}
 if(!wait_state(s,SERVICE_STOPPED)){std::printf("STOPPED_FAIL\n");return 15;}
 if(!DeleteService(s)){std::printf("DELETE_FAIL %lu\n",GetLastError());return 16;}
 CloseServiceHandle(s); CloseServiceHandle(scm); std::printf("PRIME_W6_SERVICE_OK\n"); return 0; }
