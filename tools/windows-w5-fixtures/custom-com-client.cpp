#include "custom-com.h"
#include <cstdio>
int main() {
    HRESULT hr=CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED); if(FAILED(hr)) return 10;
    IPrimeW5Fixture* obj=nullptr;
    hr=CoCreateInstance(CLSID_PrimeW5Fixture,nullptr,CLSCTX_INPROC_SERVER,IID_IPrimeW5Fixture,(void**)&obj);
    if(FAILED(hr) || !obj) { std::printf("PRIME_W5_ACTIVATE_FAIL 0x%08lx\n",(unsigned long)hr); CoUninitialize(); return 11; }
    LONG value=0; hr=obj->GetValue(&value); obj->Release(); CoUninitialize();
    if(FAILED(hr) || value!=4242) return 12;
    std::printf("PRIME_W5_CUSTOM_COM_OK VALUE=%ld\n",value); return 0;
}
