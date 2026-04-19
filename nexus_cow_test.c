#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
#include <sys/wait.h>

int main() {
    size_t size = 16384; // 16KB
    // Create a private anonymous mapping (COW behavior)
    char *vmo = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    
    if (vmo == MAP_FAILED) {
        perror("mmap failed");
        return 1;
    }

    // Write 'ORIGINAL_CORE_DNA' at offset 0
    strcpy(vmo, "ORIGINAL_CORE_DNA");

    pid_t pid = fork();
    if (pid < 0) {
        perror("fork failed");
        return 1;
    }

    if (pid == 0) {
        // CHILD PROCESS (The Clone)
        // Write 'MODIFIED_GHOST_DNA' at offset 0
        strcpy(vmo, "MODIFIED_GHOST_DNA");
        printf("CLONE_OFFSET_0_READ: %s\n", vmo);
        exit(0);
    } else {
        // PARENT PROCESS
        int status;
        waitpid(pid, &status, 0);
        
        printf("PARENT_OFFSET_0_READ: %s\n", vmo);
        
        if (strcmp(vmo, "ORIGINAL_CORE_DNA") == 0) {
            printf("REPORT: Parent remained unchanged. COW successful.\n");
        } else {
            printf("REPORT: Parent was modified. COW failed.\n");
        }
    }

    munmap(vmo, size);
    return 0;
}
