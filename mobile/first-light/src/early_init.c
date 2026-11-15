/*
 * Prime Mobile MOB-1: generic ARM64 Linux early-init observation fixture.
 * This is deliberately NOT Prime Core, a Samsung ROM, an ARM emulator or a
 * production PID 1. It is an observable, no-device/no-network Linux boot probe.
 * All observations come from the guest kernel and can be missing/unknown.
 *
 * SPDX-License-Identifier: MIT
 */
#if !defined(__aarch64__)
#error "MOB-1 early init is for AArch64 only"
#endif

typedef unsigned long usize;
typedef long isize;

/* Freestanding compiler-generated zero initialization; no libc dependency. */
void *memset(void *dst, int value, usize bytes) {
    volatile unsigned char *p = (volatile unsigned char *)dst;
    for (usize i = 0; i < bytes; ++i) p[i] = (unsigned char)value;
    return dst;
}

/* Linux arm64 syscall numbers (asm-generic/unistd.h). */
enum { SYS_MOUNT = 40, SYS_OPENAT = 56, SYS_CLOSE = 57, SYS_READ = 63,
       SYS_WRITE = 64, SYS_NANOSLEEP = 101, SYS_UNAME = 160, SYS_GETPID = 172 };
enum { AT_FDCWD = -100, O_RDONLY = 0 };

static inline isize syscall5(isize number, isize a0, isize a1,
                             isize a2, isize a3, isize a4) {
    register isize x0 __asm__("x0") = a0;
    register isize x1 __asm__("x1") = a1;
    register isize x2 __asm__("x2") = a2;
    register isize x3 __asm__("x3") = a3;
    register isize x4 __asm__("x4") = a4;
    register isize x8 __asm__("x8") = number;
    __asm__ volatile("svc #0" : "+r"(x0)
                     : "r"(x1), "r"(x2), "r"(x3), "r"(x4), "r"(x8)
                     : "memory");
    return x0;
}

static usize length(const char *s) {
    usize n = 0;
    while (s[n] && n < 4096) ++n;
    return n;
}

static void emit(const char *message) {
    usize offset = 0, count = length(message);
    while (offset < count) {
        isize result = syscall5(SYS_WRITE, 1, (isize)(message + offset),
                                (isize)(count - offset), 0, 0);
        if (result <= 0) break; /* fail silently when no console is available */
        offset += (usize)result;
    }
}

static void emit_sanitized(const char *s, usize limit) {
    char out[129];
    usize i = 0;
    if (limit > 128) limit = 128;
    while (i < limit && s[i] != 0) {
        unsigned char c = (unsigned char)s[i];
        if ((c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') ||
            (c >= 'A' && c <= 'Z') || c == '-' || c == '_' ||
            c == '.' || c == ' ' || c == '/' || c == '+' || c == ',')
            out[i] = (char)c;
        else out[i] = '?';
        ++i;
    }
    out[i] = 0;
    emit(out);
}

struct guest_utsname {
    char sysname[65]; char nodename[65]; char release[65];
    char version[65]; char machine[65]; char domainname[65];
};
struct guest_timespec { isize seconds; isize nanoseconds; };

static isize kernel_read(const char *path, char *data, usize capacity) {
    isize fd = syscall5(SYS_OPENAT, AT_FDCWD, (isize)path, O_RDONLY, 0, 0);
    if (fd < 0) return -1;
    isize count = syscall5(SYS_READ, fd, (isize)data, (isize)(capacity - 1), 0, 0);
    (void)syscall5(SYS_CLOSE, fd, 0, 0, 0, 0);
    if (count < 0) return -1;
    data[count] = 0;
    return count;
}

/* Parsed from the guest Linux kernel, never a model-generated estimate.
 * Missing fields remain explicitly UNKNOWN; swap is never physical RAM. */
static void emit_meminfo_field(const char *content, usize size,
                               const char *key, const char *label) {
    emit("PRIME_MOB1_V0|"); emit(label); emit("=");
    usize keylen = length(key);
    for (usize pos = 0; pos < size;) {
        usize end = pos;
        while (end < size && content[end] != '\n') ++end;
        usize i = 0;
        while (i < keylen && pos+i < end && content[pos+i] == key[i]) ++i;
        if (i == keylen) {
            usize k = pos+keylen;
            while (k < end && (content[k]==' ' || content[k]=='\t')) ++k;
            usize start = k;
            while (k < end && content[k]>='0' && content[k]<='9' && k-start < 20) ++k;
            if (k > start) {
                usize units = k;
                while (units < end && (content[units]==' ' || content[units]=='\t')) ++units;
                if (units+2 == end && content[units]=='k' && content[units+1]=='B') {
                    char digits[21]; usize n=0;
                    while (start < k) digits[n++] = content[start++];
                    digits[n]=0;
                    emit(digits); emit("\n"); return;
                }
            }
        }
        pos = end < size ? end+1 : size;
    }
    emit("UNKNOWN\n");
}

static void probe_memory(void) {
    char data[4096];
    isize n = kernel_read("/proc/meminfo",data,sizeof(data));
    usize size = n > 0 ? (usize)n : 0;
    emit_meminfo_field(data,size,"MemTotal:","mem_total_kib");
    emit_meminfo_field(data,size,"MemAvailable:","mem_available_kib");
    emit_meminfo_field(data,size,"SwapTotal:","swap_total_kib");
    emit_meminfo_field(data,size,"SwapFree:","swap_free_kib");
}

static void probe_memory_psi(void) {
    char data[1024];
    isize n = kernel_read("/proc/pressure/memory",data,sizeof(data));
    emit("PRIME_MOB1_V0|psi_memory_some_avg10=");
    if (n <= 0) { emit("UNKNOWN\n"); return; }
    const char key[] = "some avg10=";
    const usize keylen=sizeof(key)-1;
    if ((usize)n < keylen) { emit("UNKNOWN\n"); return; }
    for (usize j=0;j<keylen;++j)
        if (data[j]!=key[j]) { emit("UNKNOWN\n"); return; }
    char avg[17]; usize i=0, offset=keylen;
    while (offset<(usize)n && i<sizeof(avg)-1 &&
           ((data[offset]>='0' && data[offset]<='9') || data[offset]=='.')) {
        avg[i++]=data[offset++];
    }
    if (!i || offset >= (usize)n || data[offset]!=' ' || i>8) {
        emit("UNKNOWN\n"); return;
    }
    avg[i]=0; emit(avg); emit("\n");
}

static void probe_zswap(void) {
    char data[16];
    isize n=kernel_read("/sys/module/zswap/parameters/enabled",data,sizeof(data));
    emit("PRIME_MOB1_V0|zswap_state=");
    if (n>0 && (data[0]=='Y' || data[0]=='y' || data[0]=='1')) emit("ENABLED\n");
    else if (n>0 && (data[0]=='N' || data[0]=='n' || data[0]=='0')) emit("DISABLED\n");
    else emit("UNKNOWN\n");
}

static void probe_device_tree(void) {
    char data[129];
    isize count = kernel_read("/sys/firmware/devicetree/base/model", data, sizeof(data));
    emit("PRIME_MOB1_V0|device_tree_model=");
    if (count <= 0) emit("UNKNOWN");
    else emit_sanitized(data, (usize)count);
    emit("\n");
}

static void early_init(void) {
    emit("PRIME_MOB1_V0|stage=INIT_ENTERED\n");
    emit("PRIME_MOB1_V0|compiled_arch=aarch64\n");
    isize pid = syscall5(SYS_GETPID, 0, 0, 0, 0, 0);
    emit(pid == 1 ? "PRIME_MOB1_V0|pid1=true\n" : "PRIME_MOB1_V0|pid1=false\n");
    struct guest_utsname u = {0};
    isize uname_status = syscall5(SYS_UNAME, (isize)&u, 0, 0, 0, 0);
    emit("PRIME_MOB1_V0|kernel_machine=");
    if (uname_status == 0) emit_sanitized(u.machine, sizeof(u.machine));
    else emit("UNKNOWN");
    emit("\n");
    isize pm = syscall5(SYS_MOUNT, (isize)"proc", (isize)"/proc", (isize)"proc", 0, 0);
    emit(pm == 0 ? "PRIME_MOB1_V0|proc_mount=AVAILABLE\n" : "PRIME_MOB1_V0|proc_mount=UNAVAILABLE\n");
    isize sm = syscall5(SYS_MOUNT, (isize)"sysfs", (isize)"/sys", (isize)"sysfs", 0, 0);
    emit(sm == 0 ? "PRIME_MOB1_V0|sys_mount=AVAILABLE\n" : "PRIME_MOB1_V0|sys_mount=UNAVAILABLE\n");
    probe_memory();
    probe_memory_psi();
    probe_zswap();
    probe_device_tree();
    emit("PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED\n");
    struct guest_timespec pause = {1, 0};
    for (;;) (void)syscall5(SYS_NANOSLEEP, (isize)&pause, 0, 0, 0, 0);
}

/* The Linux kernel supplies a valid initial stack to /init. No libc/runtime. */
__attribute__((noreturn, used)) void _start(void) {
    early_init();
    __builtin_unreachable();
}
