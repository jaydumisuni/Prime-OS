#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static SERVICE_STATUS_HANDLE g_handle=nullptr; static HANDLE g_stop=nullptr;
void set_state(DWORD s,DWORD err=NO_ERROR){ SERVICE_STATUS st{}; st.dwServiceType=SERVICE_WIN32_OWN_PROCESS; st.dwCurrentState=s; st.dwControlsAccepted=(s==SERVICE_RUNNING)?SERVICE_ACCEPT_STOP:0; st.dwWin32ExitCode=err; SetServiceStatus(g_handle,&st); }
DWORD WINAPI ctrl(DWORD c,DWORD,LPVOID,LPVOID){ if(c==SERVICE_CONTROL_STOP){ set_state(SERVICE_STOP_PENDING); SetEvent(g_stop); return NO_ERROR;} return ERROR_CALL_NOT_IMPLEMENTED; }
void WINAPI main_service(DWORD,LPWSTR*){ g_handle=RegisterServiceCtrlHandlerExW(L"PrimeW6Proof",ctrl,nullptr); if(!g_handle) return; g_stop=CreateEventW(nullptr,TRUE,FALSE,nullptr); if(!g_stop){ set_state(SERVICE_STOPPED,GetLastError()); return;} set_state(SERVICE_RUNNING); WaitForSingleObject(g_stop,INFINITE); set_state(SERVICE_STOPPED); CloseHandle(g_stop); }
int wmain(){ SERVICE_TABLE_ENTRYW table[]={{(LPWSTR)L"PrimeW6Proof",main_service},{nullptr,nullptr}}; return StartServiceCtrlDispatcherW(table)?0:(int)GetLastError(); }
