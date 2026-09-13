#pragma once
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <unknwn.h>

// {5D5B74A4-FA57-4F52-9826-5918955DF021}
static const CLSID CLSID_PrimeW5Fixture = {0x5d5b74a4,0xfa57,0x4f52,{0x98,0x26,0x59,0x18,0x95,0x5d,0xf0,0x21}};
// {E6481F45-0E86-45C9-9F1E-60B49766E949}
static const IID IID_IPrimeW5Fixture = {0xe6481f45,0x0e86,0x45c9,{0x9f,0x1e,0x60,0xb4,0x97,0x66,0xe9,0x49}};

struct IPrimeW5Fixture : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE GetValue(LONG* value) = 0;
};
