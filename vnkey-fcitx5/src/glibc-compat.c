/*
 * glibc-compat.c — Shim tương thích cho các symbol glibc phiên bản cao
 *
 * Khi build trên Fedora 39 (glibc 2.38) nhưng nhắm Rocky 9 (glibc 2.34),
 * các định nghĩa cục bộ này thỏa mãn các symbol từ Rust std và GCC 13
 * mà nếu không sẽ yêu cầu GLIBC_2.35+.
 *
 * Biên dịch: gcc -c -fPIC glibc-compat.c -o glibc-compat.o
 * Liên kết TRƯỚC -lc: g++ -shared ... glibc-compat.o ... -lc
 */

#define _GNU_SOURCE
#include <stdlib.h>
#include <sys/random.h>
#include <errno.h>
#include <unistd.h>

/*
 * arc4random / arc4random_buf — GLIBC_2.36
 * Dùng bởi HashMap (RandomState) của Rust std để tạo ngẫu nhiên.
 * Hiện thực dùng getrandom() (có từ GLIBC_2.25 / Linux 3.17).
 */
unsigned int arc4random(void) {
    unsigned int val;
    ssize_t ret;
    do {
        ret = getrandom(&val, sizeof(val), 0);
    } while (ret < 0 && errno == EINTR);
    if (ret != (ssize_t)sizeof(val))
        abort();
    return val;
}

void arc4random_buf(void *buf, size_t n) {
    char *p = (char *)buf;
    while (n > 0) {
        ssize_t ret = getrandom(p, n, 0);
        if (ret < 0) {
            if (errno == EINTR) continue;
            abort();
        }
        p += (size_t)ret;
        n -= (size_t)ret;
    }
}

/*
 * __isoc23_strtol — GLIBC_2.38
 * GCC 13 trên Fedora 39 chuyển hướng lời gọi strtol() sang __isoc23_strtol()
 * (ngữ nghĩa C23: chấp nhận tiền tố 0b cho base=0). Code chúng ta không cần
 * ngữ nghĩa C23, nên chuyển hướng về strtol thường.
 */
long __isoc23_strtol(const char *nptr, char **endptr, int base) {
    return strtol(nptr, endptr, base);
}

long __isoc23_strtoul(const char *nptr, char **endptr, int base) {
    return (long)strtoul(nptr, endptr, base);
}

/*
 * _dl_find_object — GLIBC_2.35
 * Dùng bởi unwinder libgcc_eh tĩnh của GCC 13 như đường tắt cho
 * xử lý ngoại lệ. Trả về -1 báo cho unwinder dùng phương án dự phòng
 * dl_iterate_phdr() (có trong mọi phiên bản glibc).
 */
struct dl_find_object;
int _dl_find_object(void *pc, struct dl_find_object *result) {
    (void)pc;
    (void)result;
    return -1;  /* không tìm thấy → dùng phương án dl_iterate_phdr */
}

/*
 * pidfd_getpid / pidfd_spawnp — GLIBC_2.39
 * Dùng bởi Rust std (process::Command) trên glibc mới để spawn/quản lý
 * tiến trình con qua pidfd. VnKey IME không spawn tiến trình nên trả lỗi
 * ENOSYS là đủ — Rust std sẽ dùng phương án dự phòng fork/exec chuẩn.
 * Đây là weak symbol nên chỉ cần cung cấp stub.
 */
#include <spawn.h>
#include <signal.h>

int pidfd_getpid(int pidfd) {
    (void)pidfd;
    errno = ENOSYS;
    return -1;
}

int pidfd_spawnp(int *pidfd, const char *file,
                 const posix_spawn_file_actions_t *file_actions,
                 const posix_spawnattr_t *attrp,
                 char *const argv[], char *const envp[]) {
    (void)pidfd;
    (void)file;
    (void)file_actions;
    (void)attrp;
    (void)argv;
    (void)envp;
    errno = ENOSYS;
    return ENOSYS;
}
