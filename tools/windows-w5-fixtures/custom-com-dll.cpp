#include "custom-com.h"
#include <new>

static HMODULE g_module = nullptr;
static LONG g_objects = 0;
static LONG g_locks = 0;

class PrimeObject final : public IPrimeW5Fixture {
    LONG refs_ = 1;
public:
    PrimeObject() { InterlockedIncrement(&g_objects); }
    ~PrimeObject() { InterlockedDecrement(&g_objects); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IPrimeW5Fixture)) {
            *out = static_cast<IPrimeW5Fixture*>(this); AddRef(); return S_OK;
        }
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override {
        ULONG n = InterlockedDecrement(&refs_); if (!n) delete this; return n;
    }
    HRESULT STDMETHODCALLTYPE GetValue(LONG* value) override {
        if (!value) return E_POINTER; *value = 4242; return S_OK;
    }
};

class Factory final : public IClassFactory {
    LONG refs_ = 1;
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** out) override {
        if (!out) return E_POINTER; *out = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory)) {
            *out = static_cast<IClassFactory*>(this); AddRef(); return S_OK;
        }
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n=InterlockedDecrement(&refs_); if(!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID riid, void** out) override {
        if (outer) return CLASS_E_NOAGGREGATION;
        PrimeObject* obj = new(std::nothrow) PrimeObject(); if(!obj) return E_OUTOFMEMORY;
        HRESULT hr=obj->QueryInterface(riid,out); obj->Release(); return hr;
    }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override {
        if(lock) InterlockedIncrement(&g_locks); else InterlockedDecrement(&g_locks); return S_OK;
    }
};

extern "C" BOOL WINAPI DllMain(HINSTANCE h, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) { g_module = h; DisableThreadLibraryCalls(h); }
    return TRUE;
}
extern "C" __declspec(dllexport) HRESULT WINAPI DllGetClassObject(REFCLSID clsid, REFIID riid, void** out) {
    if (!IsEqualCLSID(clsid, CLSID_PrimeW5Fixture)) return CLASS_E_CLASSNOTAVAILABLE;
    Factory* f=new(std::nothrow) Factory(); if(!f) return E_OUTOFMEMORY;
    HRESULT hr=f->QueryInterface(riid,out); f->Release(); return hr;
}
extern "C" __declspec(dllexport) HRESULT WINAPI DllCanUnloadNow() {
    return (g_objects==0 && g_locks==0) ? S_OK : S_FALSE;
}
