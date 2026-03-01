#include <iostream>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
extern "C" {
    int resonator_ignite(const char* path) {
        int fd = open(path, O_RDONLY);
        if (fd < 0) return -1;
        struct stat st;
        fstat(fd, &st);
        mmap(NULL, st.size, PROT_READ, MAP_SHARED, fd, 0);
        return 0;
    }
    float scan_entropy() { return 0.95f; }
}
