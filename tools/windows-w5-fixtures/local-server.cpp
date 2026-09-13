#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <oleauto.h>
#include <cstdio>
#include <cwchar>
#include <new>

// {4B733C73-893F-4A4F-93F4-E8E8A101A5B2}
static const CLSID CLSID_PrimeW5Local = {0x4b733c73,0x893f,0x4a4f,{0x93,0xf4,0xe8,0xe8,0xa1,0x01,0xa5,0xb2}};
static HANDLE g_done = nullptr;
static LONG g_objects = 0;

class LocalObject final : public IDispatch {
    LONG refs_ = 1;
public:
    LocalObject(){ InterlockedIncrement(&g_objects); }
    ~LocalObject(){ if(InterlockedDecrement(&g_objects)==0 && g_done) SetEvent(g_done); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** out) override {
        if(!out) return E_POINTER; *out=nullptr;
        if(IsEqualIID(riid,IID_IUnknown)||IsEqualIID(riid,IID_IDispatch)){*out=static_cast<IDispatch*>(this);AddRef();return S_OK;}
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n=InterlockedDecrement(&refs_); if(!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE GetTypeInfoCount(UINT* pctinfo) override { if(!pctinfo) return E_POINTER; *pctinfo=0; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetTypeInfo(UINT,LCID,ITypeInfo**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetIDsOfNames(REFIID, LPOLESTR* names, UINT c, LCID, DISPID* ids) override {
        if(!names||!ids||c!=1) return E_INVALIDARG;
        if(_wcsicmp(names[0],L"Value")==0){ids[0]=1;return S_OK;} return DISP_E_UNKNOWNNAME;
    }
    HRESULT STDMETHODCALLTYPE Invoke(DISPID id,REFIID,LCID,WORD flags,DISPPARAMS*,VARIANT* result,EXCEPINFO*,UINT*) override {
        if(id!=1 || !(flags&(DISPATCH_PROPERTYGET|DISPATCH_METHOD))) return DISP_E_MEMBERNOTFOUND;
        if(!result) return E_POINTER; VariantInit(result); result->vt=VT_I4; result->lVal=777; return S_OK;
    }
};

class Factory final : public IClassFactory {
    LONG refs_=1;
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid,void** out) override { if(!out)return E_POINTER;*out=nullptr;if(IsEqualIID(riid,IID_IUnknown)||IsEqualIID(riid,IID_IClassFactory)){*out=static_cast<IClassFactory*>(this);AddRef();return S_OK;}return E_NOINTERFACE; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n=InterlockedDecrement(&refs_);if(!n)delete this;return n; }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer,REFIID riid,void** out) override { if(outer)return CLASS_E_NOAGGREGATION;auto* o=new(std::nothrow) LocalObject();if(!o)return E_OUTOFMEMORY;HRESULT hr=o->QueryInterface(riid,out);o->Release();return hr; }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL) override { return S_OK; }
};

int wmain(int, wchar_t**) {
    HRESULT hr=CoInitializeEx(nullptr,COINIT_MULTITHREADED); if(FAILED(hr)) return 20;
    g_done=CreateEventW(nullptr,TRUE,FALSE,nullptr); if(!g_done){CoUninitialize();return 21;}
    auto* f=new(std::nothrow) Factory(); if(!f){CloseHandle(g_done);CoUninitialize();return 22;}
    DWORD cookie=0; hr=CoRegisterClassObject(CLSID_PrimeW5Local,f,CLSCTX_LOCAL_SERVER,REGCLS_MULTIPLEUSE,&cookie); f->Release();
    if(FAILED(hr)){std::printf("PRIME_W5_LOCAL_REGISTER_FAIL 0x%08lx\n",(unsigned long)hr);CloseHandle(g_done);CoUninitialize();return 23;}
    std::printf("PRIME_W5_LOCAL_SERVER_READY\n"); std::fflush(stdout);
    DWORD wait=WaitForSingleObject(g_done,30000);
    CoRevokeClassObject(cookie); CloseHandle(g_done); CoUninitialize();
    if(wait!=WAIT_OBJECT_0){std::printf("PRIME_W5_LOCAL_SERVER_TIMEOUT\n");return 24;}
    std::printf("PRIME_W5_LOCAL_SERVER_DONE\n"); return 0;
}
