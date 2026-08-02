#include <stdio.h>
#include <stdint.h>
#include <string.h>

// Atomic verification logic for ARM64 memory constraints
int verify_checksum(uint8_t *data, size_t len, uint32_t expected) {
    uint32_t checksum = 0;
    for (size_t i = 0; i < len; i++) {
        checksum ^= data[i];
    }
    return checksum == expected;
}

int main() {
    uint8_t buffer[] = {0xDE, 0xAD, 0xBE, 0xEF};
    uint32_t expected = 0xDE ^ 0xAD ^ 0xBE ^ 0xEF;
    if (verify_checksum(buffer, 4, expected)) {
        printf("Verification Successful\n");
        return 0;
    }
    return 1;
}