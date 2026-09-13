#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winsvc.h>
#include <cstdio>
#include <cwchar>
#ifndef PRIME_W6_ACTION
#define PRIME_W6_ACTION 0
#endif
static const wchar_t* NAME=L"PrimeW6Proof";
static bool wait_state(SC_HANDLE s,DWORD want,DWORD* pid=nullptr){for(int i=0;i<150;i++){SERVICE_STATUS_PROCESS q{};DWORD n=0;if(!QueryServiceStatusEx(s,SC_STATUS_PROCESS_INFO,(LPBYTE)&q,sizeof(q),&n))return false;if(q.dwCurrentState==want){if(pid)*pid=q.dwProcessId;return true;}Sleep(100);}return false;}
static bool sibling_service(wchar_t* out,size_t cap){DWORD n=GetModuleFileNameW(nullptr,out,(DWORD)cap);if(!n||n>=cap)return false;wchar_t* slash=wcsrchr(out,L'\\');if(!slash)return false;
#ifdef _WIN64
 const wchar_t* exe=L"service-x64.exe";
#else
 const wchar_t* exe=L"service-x86.exe";
#endif
 wcscpy_s(slash+1,cap-(slash+1-out),exe);return true;}
static SC_HANDLE open_scm(){return OpenSCManagerW(nullptr,nullptr,SC_MANAGER_ALL_ACCESS);}
static SC_HANDLE open_service(SC_HANDLE scm,DWORD access=SERVICE_ALL_ACCESS){return OpenServiceW(scm,NAME,access);}
static bool remove_if_exists(SC_HANDLE scm){SC_HANDLE s=open_service(scm);if(!s){return GetLastError()==ERROR_SERVICE_DOES_NOT_EXIST;}SERVICE_STATUS st{};ControlService(s,SERVICE_CONTROL_STOP,&st);wait_state(s,SERVICE_STOPPED);BOOL ok=DeleteService(s);DWORD e=ok?ERROR_SUCCESS:GetLastError();CloseServiceHandle(s);return ok||e==ERROR_SERVICE_MARKED_FOR_DELETE;}
static SC_HANDLE create_service(SC_HANDLE scm,const wchar_t* path,DWORD start=SERVICE_DEMAND_START){return CreateServiceW(scm,NAME,L"Prime W6 Proof",SERVICE_ALL_ACCESS,SERVICE_WIN32_OWN_PROCESS,start,SERVICE_ERROR_NORMAL,path,nullptr,nullptr,nullptr,nullptr,nullptr);}
int wmain(){wchar_t path[MAX_PATH];if(!sibling_service(path,MAX_PATH))return 2;SC_HANDLE scm=open_scm();if(!scm){std::printf("SCM_OPEN_FAIL %lu\n",GetLastError());return 10;}
#if PRIME_W6_ACTION==1
 remove_if_exists(scm);SC_HANDLE s=create_service(scm,path);if(!s){std::printf("INSTALL_FAIL %lu\n",GetLastError());return 11;}CloseServiceHandle(s);CloseServiceHandle(scm);std::printf("PRIME_W6_INSTALL_OK\n");return 0;
#elif PRIME_W6_ACTION==2
 SC_HANDLE s=open_service(scm,SERVICE_QUERY_STATUS);if(!s){std::printf("QUERY_PRESENT_FAIL %lu\n",GetLastError());return 12;}CloseServiceHandle(s);CloseServiceHandle(scm);std::printf("PRIME_W6_PRESENT_OK\n");return 0;
#elif PRIME_W6_ACTION==3
 SC_HANDLE s=open_service(scm,SERVICE_QUERY_STATUS);if(s){CloseServiceHandle(s);CloseServiceHandle(scm);std::printf("UNEXPECTED_PRESENT\n");return 13;}DWORD e=GetLastError();CloseServiceHandle(scm);if(e!=ERROR_SERVICE_DOES_NOT_EXIST){std::printf("ABSENT_FAIL %lu\n",e);return 14;}std::printf("PRIME_W6_ABSENT_OK\n");return 0;
#elif PRIME_W6_ACTION==4
 bool ok=remove_if_exists(scm);CloseServiceHandle(scm);if(!ok)return 15;std::printf("PRIME_W6_CLEANUP_OK\n");return 0;
#elif PRIME_W6_ACTION==5
 remove_if_exists(scm);SC_HANDLE s=create_service(scm,path);if(!s){std::printf("CREATE_FAIL %lu\n",GetLastError());return 20;}
 if(!StartServiceW(s,0,nullptr)||!wait_state(s,SERVICE_RUNNING)){std::printf("START1_FAIL %lu\n",GetLastError());return 21;}DWORD pid=0;if(!wait_state(s,SERVICE_RUNNING,&pid)||!pid)return 22;HANDLE h=OpenProcess(PROCESS_TERMINATE|SYNCHRONIZE,FALSE,pid);if(!h||!TerminateProcess(h,99)){std::printf("KILL_FAIL %lu\n",GetLastError());return 23;}WaitForSingleObject(h,5000);CloseHandle(h);if(!wait_state(s,SERVICE_STOPPED)){std::printf("KILL_STOP_FAIL\n");return 24;}if(!StartServiceW(s,0,nullptr)||!wait_state(s,SERVICE_RUNNING)){std::printf("RESTART_FAIL %lu\n",GetLastError());return 25;}SERVICE_STATUS st{};ControlService(s,SERVICE_CONTROL_STOP,&st);if(!wait_state(s,SERVICE_STOPPED))return 26;std::printf("PRIME_W6_KILL_RECOVERY_OK\n");
 wchar_t missing[MAX_PATH];wcscpy_s(missing,path);wcscat_s(missing,L".missing");if(!ChangeServiceConfigW(s,SERVICE_NO_CHANGE,SERVICE_NO_CHANGE,SERVICE_NO_CHANGE,missing,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr))return 27;BOOL bad=StartServiceW(s,0,nullptr);DWORD be=bad?ERROR_SUCCESS:GetLastError();if(bad){ControlService(s,SERVICE_CONTROL_STOP,&st);wait_state(s,SERVICE_STOPPED);return 28;}std::printf("PRIME_W6_MISSING_FAIL_CLOSED %lu\n",be);
 if(!ChangeServiceConfigW(s,SERVICE_NO_CHANGE,SERVICE_DISABLED,SERVICE_NO_CHANGE,path,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr))return 29;bad=StartServiceW(s,0,nullptr);be=bad?ERROR_SUCCESS:GetLastError();if(bad)return 30;if(be!=ERROR_SERVICE_DISABLED){std::printf("DISABLED_WRONG_ERROR %lu\n",be);return 31;}std::printf("PRIME_W6_DISABLED_FAIL_CLOSED %lu\n",be);
 if(!ChangeServiceConfigW(s,SERVICE_NO_CHANGE,SERVICE_DEMAND_START,SERVICE_NO_CHANGE,path,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr))return 32;for(int i=0;i<3;i++){if(!StartServiceW(s,0,nullptr)||!wait_state(s,SERVICE_RUNNING))return 33;ControlService(s,SERVICE_CONTROL_STOP,&st);if(!wait_state(s,SERVICE_STOPPED))return 34;}if(!DeleteService(s))return 35;CloseServiceHandle(s);CloseServiceHandle(scm);std::printf("PRIME_W6_ADVERSARIAL_OK\n");return 0;
#else
 CloseServiceHandle(scm);return 99;
#endif
}
