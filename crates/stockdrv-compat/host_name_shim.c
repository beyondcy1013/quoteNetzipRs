#include <stdint.h>

int __attribute__((__stdcall__)) GetHostNameW(uint16_t *buffer, int length) {
    static const char name[] = "localhost";
    int i;
    if (!buffer || length <= 0) return -1;
    for (i = 0; name[i]; ++i) {
        if (i + 1 >= length) return -1;
        buffer[i] = (uint16_t)(unsigned char)name[i];
    }
    buffer[i] = 0;
    return 0;
}
