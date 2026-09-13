#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <oleauto.h>
#include <cstdio>
static const CLSID CLSID_PrimeW5Local = {0x4b733c73,0x893f,0x4a4f,{0x93,0xf4,0xe8,0xe8,0xa1,0x01,0xa5,0xb2}};
int main(){
    HRESULT hr=CoInitializeEx(nullptr,COINIT_MULTITHREADED);if(FAILED(hr))return 10;
    IDispatch* d=nullptr;hr=CoCreateInstance(CLSID_PrimeW5Local,nullptr,CLSCTX_LOCAL_SERVER,IID_IDispatch,(void**)&d);
    if(FAILED(hr)||!d){std::printf("PRIME_W5_LOCAL_FAIL 0x%08lx\n",(unsigned long)hr);CoUninitialize();return 11;}
    OLECHAR* name=const_cast<OLECHAR*>(L"Value");DISPID id=0;hr=d->GetIDsOfNames(IID_NULL,&name,1,LOCALE_USER_DEFAULT,&id);if(FAILED(hr)){d->Release();CoUninitialize();return 12;}
    DISPPARAMS dp{};VARIANT v;VariantInit(&v);hr=d->Invoke(id,IID_NULL,LOCALE_USER_DEFAULT,DISPATCH_PROPERTYGET,&dp,&v,nullptr,nullptr);d->Release();CoUninitialize();
    if(FAILED(hr)||v.vt!=VT_I4||v.lVal!=777){VariantClear(&v);return 13;}LONG value=v.lVal;VariantClear(&v);std::printf("PRIME_W5_LOCAL_OK VALUE=%ld\n",value);return 0;
}
