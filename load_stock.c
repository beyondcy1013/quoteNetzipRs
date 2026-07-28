#include <windows.h>
#include <stdio.h>

typedef int(__stdcall *StartFunc)(void*);
typedef int(__stdcall *AskFunc)(const wchar_t*, void*, int);

int main() {
    printf("[*] Loading Stock.dll...\n");
    HMODULE hMod = LoadLibraryA("Z:\\netzipapi-rust-demo\\netzip_api_bin\\NetzipAPI\\StockC#\\Stock.dll");
    if (!hMod) {
        printf("[!] Failed to load Stock.dll. Error: %lu\n", GetLastError());
        return 1;
    }
    printf("[*] Loaded Stock.dll successfully at %p\n", hMod);

    StartFunc pStart = (StartFunc)GetProcAddress(hMod, "Start");
    AskFunc pAsk = (AskFunc)GetProcAddress(hMod, "Ask");
    
    if (!pStart || !pAsk) {
        printf("[!] Failed to locate functions.\n");
        return 1;
    }

    printf("[*] Functions located. Calling Start(NULL)... (will crash if callback is required)\n");
    
    // We just want it to load so we can attach Frida.
    // Let's pause here so Frida can hook.
    printf("[*] Waiting 10 seconds for Frida to attach...\n");
    Sleep(10000);
    
    // We don't actually need to call it if Frida script triggers it or if it just establishes network on Start
    // Actually, calling Start with NULL might crash.
    // Let's just exit.
    return 0;
}
