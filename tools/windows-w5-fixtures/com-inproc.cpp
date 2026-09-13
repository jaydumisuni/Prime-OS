#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <oleauto.h>
#include <cstdio>

int main() {
    HRESULT hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    if (FAILED(hr)) { std::printf("COINIT_FAIL 0x%08lx\n", (unsigned long)hr); return 10; }
    CLSID clsid{};
    hr = CLSIDFromProgID(L"Scripting.Dictionary", &clsid);
    if (FAILED(hr)) { std::printf("CLSID_FAIL 0x%08lx\n", (unsigned long)hr); CoUninitialize(); return 11; }
    IDispatch* dispatch = nullptr;
    hr = CoCreateInstance(clsid, nullptr, CLSCTX_INPROC_SERVER, IID_IDispatch, (void**)&dispatch);
    if (FAILED(hr) || !dispatch) { std::printf("COCREATE_FAIL 0x%08lx\n", (unsigned long)hr); CoUninitialize(); return 12; }
    OLECHAR* name = const_cast<OLECHAR*>(L"Count");
    DISPID dispid{};
    hr = dispatch->GetIDsOfNames(IID_NULL, &name, 1, LOCALE_USER_DEFAULT, &dispid);
    if (FAILED(hr)) { std::printf("GETID_FAIL 0x%08lx\n", (unsigned long)hr); dispatch->Release(); CoUninitialize(); return 13; }
    DISPPARAMS params{};
    VARIANT result; VariantInit(&result);
    hr = dispatch->Invoke(dispid, IID_NULL, LOCALE_USER_DEFAULT, DISPATCH_PROPERTYGET, &params, &result, nullptr, nullptr);
    if (FAILED(hr)) { std::printf("INVOKE_FAIL 0x%08lx\n", (unsigned long)hr); dispatch->Release(); CoUninitialize(); return 14; }
    long count = (result.vt == VT_I4) ? result.lVal : -1;
    VariantClear(&result);
    dispatch->Release();
    CoUninitialize();
    if (count != 0) { std::printf("COUNT_FAIL %ld\n", count); return 15; }
    std::printf("PRIME_W5_INPROC_OK COUNT=%ld\n", count);
    return 0;
}
