/* Test-only deterministic read fault injection, never linked into NIL itself.
 * Both Rust File::read and C fread see the same finite file plus EIO at offset F.
 * C fread is interposed too: Darwin libc's internal read is not interposable.
 * This fixture only uses size=1, an unbuffered file input, and no seeks by clients.
 */
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#include <string.h>
#include <sys/types.h>
#include <sys/syscall.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <limits.h>
#ifndef __APPLE__
#include <dlfcn.h>
#endif

static unsigned fixture_calls, failures;
static uint64_t prefix, failure_request;
static int fixture(int fd) {
    char path[4096];
#ifdef __APPLE__
    if (fcntl(fd, F_GETPATH, path) != 0) return 0;
#else
    char link[64];
    snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
    ssize_t n = readlink(link, path, sizeof(path)-1);
    if (n < 0) return 0;
    path[n] = 0;
#endif
    const char *name = strrchr(path, '/');
    return name && strcmp(name + 1, "nil-quota-fault.bin") == 0;
}
static ssize_t audited_read(int fd, void *buffer, size_t count) {
    if (!fixture(fd)) return (ssize_t)syscall(SYS_read, fd, buffer, count);
    ++fixture_calls;
    if (!count) return 0;
    off_t offset = lseek(fd, 0, SEEK_CUR);
    const char *raw = getenv("NIL_AUDIT_FAIL_AFTER");
    uint64_t limit = raw ? strtoull(raw, NULL, 10) : UINT64_MAX;
    if (offset < 0) abort();
    if ((uint64_t)offset >= limit) { ++failures; failure_request = count; errno = EIO; return -1; }
    if (limit - (uint64_t)offset < count) count = (size_t)(limit - (uint64_t)offset);
    ssize_t received = pread(fd, buffer, count, offset);
    if (received > 0) {
        if (lseek(fd, offset + received, SEEK_SET) < 0) abort();
        prefix += (uint64_t)received;
    }
    return received;
}
static size_t audited_fread(void *buffer, size_t size, size_t count, FILE *stream) {
    if (!fixture(fileno(stream))) {
#ifdef __APPLE__
        abort(); /* The Darwin audit intentionally observes only fixture fread. */
#else
        size_t (*original)(void *, size_t, size_t, FILE *) = dlsym(RTLD_NEXT, "fread");
        if (!original) abort();
        return original(buffer, size, count, stream);
#endif
    }
    if (size != 1) abort();
    size_t total = 0;
    while (total < count) {
        ssize_t received = audited_read(fileno(stream), (char *)buffer + total, count - total);
#ifdef __APPLE__
        if (received < 0) { stream->_flags |= __SERR; break; }
        if (!received) { stream->_flags |= __SEOF; break; }
#else
        if (received < 0) { stream->_flags |= _IO_ERR_SEEN; break; }
        if (!received) { stream->_flags |= _IO_EOF_SEEN; break; }
#endif
        total += (size_t)received;
    }
    return total;
}
#ifdef __APPLE__
__attribute__((used)) static struct { const void *replacement, *original; }
read_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)audited_read, (const void *)read
};
__attribute__((used)) static struct { const void *replacement, *original; }
fread_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)audited_fread, (const void *)fread
};
#else
ssize_t read(int fd, void *buffer, size_t count) { return audited_read(fd, buffer, count); }
size_t fread(void *buffer, size_t size, size_t count, FILE *stream) {
    return audited_fread(buffer, size, count, stream);
}
#endif
__attribute__((destructor)) static void receipt(void) {
    const char *path = getenv("NIL_AUDIT_TRACE");
    if (!path) return;
    FILE *file = fopen(path, "w");
    if (!file) abort();
    fprintf(file, "{\"read_calls\":%u,\"prefix_bytes\":%llu,\"EIO_returns\":%u,\"failure_request_bytes\":%llu}\n",
            fixture_calls, (unsigned long long)prefix, failures, (unsigned long long)failure_request);
    fclose(file);
}
