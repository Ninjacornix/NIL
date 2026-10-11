/* Single worker: arena ownership remains single-threaded. Stack exhaustion is
 * an environment failure, never a claim about exact language call depth. */
#include <pthread.h>
#include <signal.h>
#include <sys/mman.h>
#include <unistd.h>

static uintptr_t nil_guard_low, nil_guard_high;
static size_t nil_guard_page;
static struct sigaction nil_old_segv, nil_old_bus;

static void nil_stack_fault(int signal_number, siginfo_t *info, void *context) {
    (void)context;
    uintptr_t address = (uintptr_t)info->si_addr;
    if ((address >= nil_guard_low && address - nil_guard_low < nil_guard_page) ||
        (address >= nil_guard_high && address - nil_guard_high < nil_guard_page)) {
        static const char message[] = "E008 call depth/stack exhausted\n";
        (void)write(STDERR_FILENO, message, sizeof(message) - 1);
        _exit(1);
    }
    /* Re-execute a synchronous fault under the previous disposition. In
     * particular, keep sanitizer diagnostics and genuine memory bugs visible. */
    (void)sigaction(signal_number,
        signal_number == SIGSEGV ? &nil_old_segv : &nil_old_bus, NULL);
    if (info->si_code <= 0) (void)raise(signal_number);
}

static void nil_stack_failure_at(const char *where) {
    fprintf(stderr, "E008 cannot allocate execution stack (%s)\n", where);
    exit(1);
}

typedef struct { int argc; char **argv; int result; } NilStackArguments;
static void *nil_stack_worker(void *opaque) {
    NilStackArguments *arguments = opaque;
    stack_t previous, alternate = {0};
    alternate.ss_size = 64 * 1024;
    alternate.ss_sp = malloc(alternate.ss_size);
    if (!alternate.ss_sp || sigaltstack(&alternate, &previous)) nil_stack_failure_at("install alternate");
    arguments->result = nil_program_main(arguments->argc, arguments->argv);
    /* Darwin validates ss_size even when disabling the alternate stack. */
    if (previous.ss_flags & SS_DISABLE) previous.ss_size = alternate.ss_size;
    if (sigaltstack(&previous, NULL)) nil_stack_failure_at("restore alternate");
    free(alternate.ss_sp);
    return NULL;
}

int main(int argc, char **argv) {
    long page = sysconf(_SC_PAGESIZE);
    size_t requested = $STACK_BYTES;
    if (page <= 0 || requested > SIZE_MAX - 3 * (size_t)page) nil_stack_failure_at(__func__);
    size_t usable = (requested + (size_t)page - 1) / (size_t)page * (size_t)page;
    size_t total = usable + 2 * (size_t)page;
    void *mapping = mmap(NULL, total, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if (mapping == MAP_FAILED) nil_stack_failure_at(__func__);
    char *base = (char *)mapping + page;
    if (mprotect(base, usable, PROT_READ | PROT_WRITE)) nil_stack_failure_at(__func__);
    nil_guard_low = (uintptr_t)mapping;
    nil_guard_high = (uintptr_t)(base + usable);
    nil_guard_page = (size_t)page;
    struct sigaction action = {0};
    action.sa_sigaction = nil_stack_fault;
    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGSEGV, &action, &nil_old_segv) ||
        sigaction(SIGBUS, &action, &nil_old_bus)) nil_stack_failure_at(__func__);
    pthread_attr_t attributes;
    if (pthread_attr_init(&attributes) || pthread_attr_setguardsize(&attributes, 0) ||
        pthread_attr_setstack(&attributes, base, usable)) nil_stack_failure_at(__func__);
    NilStackArguments arguments = {argc, argv, 1};
    pthread_t worker;
    if (pthread_create(&worker, &attributes, nil_stack_worker, &arguments)) nil_stack_failure_at(__func__);
    if (pthread_attr_destroy(&attributes) || pthread_join(worker, NULL)) nil_stack_failure_at(__func__);
    if (sigaction(SIGSEGV, &nil_old_segv, NULL) || sigaction(SIGBUS, &nil_old_bus, NULL))
        nil_stack_failure_at(__func__);
    if (munmap(mapping, total)) nil_stack_failure_at(__func__);
    return arguments.result;
}
