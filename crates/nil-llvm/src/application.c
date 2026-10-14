#include <stddef.h>
#include <assert.h>
#include <stdbool.h>
#include <sys/stat.h>
#include <unistd.h>
#include <dirent.h>
#include <errno.h>

/* Opt-in tooling: successful payload copies, once per kind at each site.
   The parallel root metadata below never changes roots or arena accounting. */
struct NilSequence;
#if NIL_EXPLAIN_COPIES
static uint64_t *nil_hint_seen;
static uint64_t nil_hint_start, nil_hint_end, nil_hint_read_start, nil_hint_read_end;
static bool nil_hint_retaining(const struct NilSequence *,uint64_t *,uint64_t *);
void nil_copy_site(uint64_t *seen,uint64_t start,uint64_t end,uint64_t read_start,uint64_t read_end) {
    nil_hint_seen=seen;nil_hint_start=start;nil_hint_end=end;
    nil_hint_read_start=read_start;nil_hint_read_end=read_end;
}
static void nil_copy_hint(const char *kind,uint64_t bytes,unsigned role,const struct NilSequence *source) {
    static const char *const kinds[]={"replace","concat","concat_right","append_right",
        "slice","map_rebuild","sort","record_get","record_set","record_concat",
        "record_concat_right","record_append_right","record_slice","map_key",
        "map_value","map_record_value","map_insert_key","map_update_value","record_update_value"};
    if(!bytes || !nil_hint_seen) return;
    unsigned index=0;while(index<sizeof(kinds)/sizeof(kinds[0]) && strcmp(kind,kinds[index])) index++;
    if(index==sizeof(kinds)/sizeof(kinds[0])) abort();
    uint64_t bit=UINT64_C(1)<<index;
    if(*nil_hint_seen & bit) return;
    *nil_hint_seen|=bit;
    fprintf(stderr,"NIL_COPY_HINT {\"kind\":\"%s\",\"bytes\":%" PRIu64 ",\"span\":",kind,bytes);
    if(nil_hint_start==UINT64_MAX) fputs("null",stderr);
    else fprintf(stderr,"[%" PRIu64 ",%" PRIu64 "]",nil_hint_start,nil_hint_end);
    fputs(",\"keeping_read\":",stderr);
    if(role!=1 || nil_hint_read_start==UINT64_MAX) fputs("null",stderr);
    else fprintf(stderr,"[%" PRIu64 ",%" PRIu64 "]",nil_hint_read_start,nil_hint_read_end);
    uint64_t start=0,end=0;
    bool retained=role==1 && nil_hint_retaining(source,&start,&end);
    fputs(",\"retaining_use\":",stderr);
    if(retained) fprintf(stderr,"[%" PRIu64 ",%" PRIu64 "]",start,end);
    else fputs("null",stderr);
    fputs(",\"precision\":\"",stderr);
    fputs(role==1 && nil_hint_read_start!=UINT64_MAX ? "direct-local-read" :
          retained ? "root-retention-use" : "unknown-alias-or-required-materialization",stderr);
    fputs("\",\"reason\":\"",stderr);
    fputs(role!=1 ? "required-materialization" :
          nil_hint_read_start!=UINT64_MAX ? "later-local-read" :
          retained ? "live-root-retention" : "unresolved-copy-cause",stderr);
    fputs("\"}\n",stderr);
}
#else
void nil_copy_site(uint64_t *seen,uint64_t start,uint64_t end,uint64_t read_start,uint64_t read_end) {
    (void)seen;(void)start;(void)end;(void)read_start;(void)read_end;
}
#define nil_copy_hint(kind, bytes, role, source) ((void)0)
#endif

/* A stable opaque allocation-family descriptor avoids macOS allocator bucket
   choices depending on emitted code addresses. Its upper summary bits are zero:
   no pointer-layout or pure-data claim is made for variable sequence tails.
   Weak imports and the SDK guard preserve the ordinary portable allocation path.
   NIL_PORTABLE_ALLOCATION is an internal fallback-test switch, not a NIL option. */
#if defined(__APPLE__)
#include <AvailabilityMacros.h>
#if MAC_OS_X_VERSION_MAX_ALLOWED >= 140000 && !defined(NIL_PORTABLE_ALLOCATION)
extern void *malloc_type_malloc(size_t, uint64_t) __attribute__((weak_import, malloc, alloc_size(1)));
extern void *malloc_type_realloc(void *, size_t, uint64_t) __attribute__((weak_import, alloc_size(2)));
#define NIL_HAS_TYPED_HEAP 1
#endif
#endif
static inline void *nil_heap_malloc(size_t bytes) {
#ifdef NIL_HAS_TYPED_HEAP
    if (malloc_type_malloc) return malloc_type_malloc(bytes, UINT64_C(0x4e494c53));
#endif
    return malloc(bytes);
}
static inline void *nil_heap_realloc(void *value, size_t bytes) {
#ifdef NIL_HAS_TYPED_HEAP
    if (malloc_type_realloc) return malloc_type_realloc(value, bytes, UINT64_C(0x4e494c53));
#endif
    return realloc(value, bytes);
}

/* Application runtime: execution-owned immutable sequences. No public pointer ABI. */
typedef struct NilSequence {
    struct NilSequence *next;
    int64_t length;
    uint64_t width;
    uint64_t roots;
    uint64_t capacity;
    unsigned char data[];
} NilSequence;
/* The native runtime is single-threaded per process: it creates no threads and each
   program runs in its own process. Plain globals avoid per-access thread-local lookup,
   which ADR 038 measured on every root observation. */
/* Sweeping costs one visit per live allocation, not per byte, so collection is
   also spaced geometrically in the allocation count. */
static uint64_t nil_alloc_count, nil_collect_floor;
/* Below this many live allocations a full sweep is cheap enough to run on byte
   pressure alone. */
#define NIL_COLLECT_EAGER_ENTRIES UINT64_C(1024)
static NilSequence *nil_allocations;
static uint64_t nil_allocated;
static uint64_t nil_live;
static uint64_t nil_random_state;
static bool nil_random_ready;
#define NIL_SEQUENCE_OVERHEAD UINT64_C(40)
#define NIL_MEMORY_LIMIT UINT64_C(67108864)
_Static_assert(offsetof(NilSequence,data)==40 && _Alignof(NilSequence)==8, "sequence data ABI mismatch");
_Static_assert(sizeof(NilSequence)==NIL_SEQUENCE_OVERHEAD, "sequence charge/layout mismatch");
typedef struct NilRoots {
    struct NilRoots *previous;
    NilSequence **slots;
    uint64_t count;
} NilRoots;
static NilRoots *nil_roots;
#if NIL_EXPLAIN_COPIES
typedef struct NilHintRoots {
    struct NilHintRoots *previous;
    NilRoots *frame;
    uint64_t (*spans)[2];
} NilHintRoots;
_Static_assert(sizeof(NilHintRoots)==24 && _Alignof(NilHintRoots)==8,"hint frame ABI mismatch");
static NilHintRoots *nil_hint_roots;
void nil_hint_frame_enter(NilRoots *frame,uint64_t (*spans)[2],NilHintRoots *storage) {
    *storage=(NilHintRoots){nil_hint_roots,frame,spans};nil_hint_roots=storage;
}
void nil_hint_frame_leave(NilHintRoots *storage) {
    if(nil_hint_roots!=storage) abort();
    nil_hint_roots=storage->previous;
}
void nil_hint_root(NilSequence **slot,uint64_t start,uint64_t end) {
    for(NilHintRoots *h=nil_hint_roots;h;h=h->previous) {
        uintptr_t base=(uintptr_t)h->frame->slots,address=(uintptr_t)slot;
        if(address>=base && (address-base)/sizeof(*slot)<h->frame->count) {
            uint64_t i=(address-base)/sizeof(*slot);
            h->spans[i][0]=start;h->spans[i][1]=end;return;
        }
    }
    abort();
}
#endif

#define NIL_RECORD_TAG (UINT64_C(1)<<63)
#ifndef NIL_RECORD_BUFFERS
#define NIL_RECORD_BUFFERS 1
#endif
static uint64_t nil_owned_size(const NilSequence *v) {
#if NIL_RECORD_BUFFERS
    if(v->width & NIL_RECORD_TAG) return NIL_SEQUENCE_OVERHEAD+(v->capacity+1)*(v->width & ~NIL_RECORD_TAG);
#endif
    return NIL_SEQUENCE_OVERHEAD+v->capacity*v->width;
}
static void nil_child_roots(NilSequence *v,bool retain);
static void nil_retain(NilSequence *v) {
    if(v && v->roots++==0) {
        nil_live+=nil_owned_size(v);
#if NIL_RECORD_BUFFERS
        nil_child_roots(v,true);
#endif
    }
}
static void nil_drop(NilSequence *v) {
    if(v && --v->roots==0) {
        nil_live-=nil_owned_size(v);
#if NIL_RECORD_BUFFERS
        nil_child_roots(v,false);
#endif
    }
}
_Static_assert(sizeof(NilRoots)==24 && _Alignof(NilRoots)==8, "root frame ABI mismatch");
/* ADR 038: a frame holds 2*count slots. slots[0,count) are the roots the code
   intends; slots[count,2*count) are the committed roots whose counts have been
   applied. Root counts and nil_live are exact only after reconciliation, so every
   reader of ->roots or nil_live, every allocation and every collection must call
   nil_roots_sync() first, except the compiler-proved observed-frame replacement
   entry point (ADR 039), whose caller maintains exact counts. Only the top frame can be unreconciled: entering a callee
   reconciles its caller. Counts are sums over final slot values, so reconciling at
   observation points yields exactly the eager counts. */
/* Set when a root slot write changes its slot; only the top frame can be dirty. */
static bool nil_roots_dirty;
static void nil_roots_sync_slow(NilRoots *frame) __attribute__((noinline));
static void nil_roots_sync_slow(NilRoots *frame) {
    NilSequence **slots=frame->slots,**committed=frame->slots+frame->count;
    for(uint64_t i=0;i<frame->count;i++)
        if(slots[i]!=committed[i]) { nil_retain(slots[i]); nil_drop(committed[i]); committed[i]=slots[i]; }
}
/* Branch-free equality check first: update loops usually store unchanged pointers. */
static inline __attribute__((always_inline)) void nil_roots_sync(void) {
    if(!nil_roots_dirty) return;
    nil_roots_dirty=false;
    NilRoots *frame=nil_roots;
    if(!frame) return;
    NilSequence **slots=frame->slots,**committed=frame->slots+frame->count;
    uintptr_t differ=0;
    for(uint64_t i=0;i<frame->count;i++) differ|=(uintptr_t)slots[i]^(uintptr_t)committed[i];
    if(differ) nil_roots_sync_slow(frame);
}
void nil_roots_observe(void) { nil_roots_sync(); }
/* Compiler proves all other slots unchanged. Copying may replace this pointer;
   commit that change immediately, preserving exact counts for the next update. */
__attribute__((always_inline)) void nil_root_store_observed(NilSequence **slot,NilSequence *value) {
    if(*slot!=value) { nil_roots_dirty=true; *slot=value; nil_roots_sync(); }
}
void *nil_roots_enter(NilSequence **slots,uint64_t count,NilRoots *frame) {
    nil_roots_sync();
    frame->previous=nil_roots; frame->slots=slots; frame->count=count;
    nil_roots=frame; return frame;
}
/* Slot updates are the only way to change execution roots. A zero-root payload
   survives transfer gaps until the next allocation/collection boundary. Counts are
   applied lazily by nil_roots_sync(). */
__attribute__((always_inline)) void nil_root_store(NilSequence **slot,NilSequence *value) {
    nil_roots_dirty|=*slot!=value; *slot=value;
}
void nil_roots_leave(NilRoots *frame) {
    if(nil_roots!=frame) abort();
    NilSequence **committed=frame->slots+frame->count;
    for(uint64_t i=0;i<frame->count;i++) { nil_drop(committed[i]); committed[i]=NULL; frame->slots[i]=NULL; }
    nil_roots=frame->previous;
    /* The caller was reconciled when this frame was entered and has not stored since. */
    nil_roots_dirty=false;
}
/* Locate the single slot holding `value` and its committed mirror, before the
   value is relocated. Callers have synced, so slot and mirror agree everywhere. */
typedef struct NilRootRef { NilSequence **slot,**mirror; } NilRootRef;
static NilRootRef nil_roots_find(NilSequence *value) {
    NilRootRef ref={NULL,NULL};
    for(NilRoots *frame=nil_roots;frame;frame=frame->previous)
        for(uint64_t i=0;i<frame->count;i++)
            if(frame->slots[i]==value) { ref.slot=&frame->slots[i]; ref.mirror=&frame->slots[frame->count+i]; }
    if(!ref.slot || *ref.mirror!=value) abort();
    return ref;
}
static void nil_collect(void) {
    nil_roots_sync();
    NilSequence **link=&nil_allocations;
    while(*link) {
        NilSequence *value=*link;
        if(!value->roots) {
            *link=value->next;
            nil_allocated-=nil_owned_size(value);
            nil_alloc_count--;
            free(value);
        } else link=&value->next;
    }
    /* Next opportunistic sweep waits until the list has grown again, which
       makes total sweep work linear in allocations rather than quadratic. */
    nil_collect_floor=nil_alloc_count;
}
/* Bound garbage as well as total physical capacity. Growing live graphs then
   trigger geometrically spaced sweeps; copy-heavy workloads do not accumulate
   large dead payloads that displace their working set from cache. */
static bool nil_should_collect(uint64_t reservation) {
    nil_roots_sync();
#if !NIL_RECORD_BUFFERS
    // Closed-world typing proves there are no heap child edges. Keep the
    // established tiny flat-arena path, including immediate allocator reuse.
    (void)reservation; return true;
#else
    uint64_t garbage=nil_allocated-nil_live;
    uint64_t budget=nil_live/4;
    if(budget<UINT64_C(65536)) budget=UINT64_C(65536);
    /* Pressure always collects. */
    if(reservation>NIL_MEMORY_LIMIT || nil_allocated>NIL_MEMORY_LIMIT-reservation) return true;
    /* Otherwise a worthwhile byte yield is required. A short allocation list
       is cheap to walk, so it still collects eagerly and byte-churning
       transforms keep their locality. Once the list is long the visit cost
       dominates, so sweeps are also spaced geometrically in the entry count;
       without that, a workload holding one small allocation per row of a
       nested collection pays O(allocations) on every sweep. */
    if(garbage<budget) return false;
    return nil_alloc_count<NIL_COLLECT_EAGER_ENTRIES
        || nil_alloc_count>=2*nil_collect_floor+64;
#endif
}
static NilSequence *nil_allocate_capacity(int64_t length, uint64_t capacity, uint64_t width, uint64_t start, uint64_t end) {
    if (length < 0) nil_fail(16,start,end);
    assert(width && capacity >= (uint64_t)length);
    if (NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD || capacity > (NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width) nil_fail(6,start,end);
    uint64_t bytes=capacity*width;
    if (nil_should_collect(bytes+NIL_SEQUENCE_OVERHEAD)) nil_collect();
    if (nil_allocated > NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    NilSequence *value=nil_heap_malloc(sizeof(*value)+(size_t)bytes);
    if (!value) nil_fail(6,start,end);
    nil_allocated+=bytes+NIL_SEQUENCE_OVERHEAD; value->next=nil_allocations; value->length=length; value->width=width; value->roots=0; value->capacity=capacity;
    nil_allocations=value; nil_alloc_count++; return value;
}
/* Sum nodes (ADR 050).

   A sum value is one pointer-sized slot. A nullary variant is unboxed as
   (tag<<3)|1: heap nodes are 8-aligned, so an odd slot is never a node and
   None/Leaf/Nil never allocate. A payload variant is a reference-counted node.

   Nodes are deliberately NOT part of the root-swept arena. Rooting a list head
   there would retain every child transitively, which is O(n) per root store
   and recursive, so a 10^6-node list could neither be rooted cheaply nor
   traversed without overflowing.

   Values are immutable and built bottom-up, so the graph is acyclic by
   construction and counting is complete. Destruction walks an intrusive
   worklist threaded through the nodes being destroyed, so dropping a
   10^6-node list allocates nothing and does not recurse. Nodes are not kept on
   any list: a counting defect therefore shows up as a real leak rather than
   being hidden by a sweep at exit. */
typedef struct NilNode {
    struct NilNode *pending;
    uint64_t refs;
    uint64_t tag;
    uint64_t slots;
    /* Bit i set means payload[i] holds a sum value, so it participates in
       counting. Variants carry at most 64 slots, so one word suffices. */
    uint64_t children;
    int64_t payload[];
} NilNode;
_Static_assert(offsetof(NilNode,payload)==40 && _Alignof(NilNode)==8,"node data ABI mismatch");
#define NIL_NODE_OVERHEAD UINT64_C(40)
static inline bool nil_node_boxed(const void *p) {
    return p && !((uintptr_t)p & UINT64_C(1));
}
static inline uint64_t nil_node_bytes(const NilNode *n) {
    return NIL_NODE_OVERHEAD+n->slots*8;
}
/* Rebuilding an immutable tree frees one node per level and allocates one per
   level, so the allocator round-trip dominates. Retired nodes of common widths
   are kept on size-bucketed lists and handed straight back out. This is not
   Perceus reuse: the payload is still rewritten, and no aliasing claim is made.
   Retired nodes stay charged to nil_allocated because the memory is still
   held; they leave nil_live. The lists are flushed before the allocation guard
   reports failure, so retired capacity never causes a spurious E013. */
#define NIL_NODE_BUCKETS 9
#define NIL_NODE_BUCKET_CAP UINT64_C(4096)
static NilNode *nil_node_retired[NIL_NODE_BUCKETS];
static uint64_t nil_node_retired_count[NIL_NODE_BUCKETS];
static void nil_node_flush(void) {
    for (unsigned b=0;b<NIL_NODE_BUCKETS;b++) {
        while (nil_node_retired[b]) {
            NilNode *node=nil_node_retired[b];
            nil_node_retired[b]=node->pending;
            nil_allocated-=nil_node_bytes(node);
            free(node);
        }
        nil_node_retired_count[b]=0;
    }
}
/* Retires a node for reissue, or frees it. Accounting for the live charge has
   already happened in the caller. */
static void nil_node_retire(NilNode *node) {
    uint64_t slots=node->slots;
    if (slots<NIL_NODE_BUCKETS && nil_node_retired_count[slots]<NIL_NODE_BUCKET_CAP) {
        node->pending=nil_node_retired[slots];
        nil_node_retired[slots]=node;
        nil_node_retired_count[slots]++;
        return;
    }
    nil_allocated-=nil_node_bytes(node);
    free(node);
}
static NilSequence *nil_allocate(int64_t length,uint64_t width,uint64_t start,uint64_t end) {
    return nil_allocate_capacity(length,length<0 ? 0 : (uint64_t)length,width,start,end);
}

void *nil_node_make(int64_t tag,int64_t slots,int64_t children,uint64_t start,uint64_t end) {
    assert(slots>0 && slots<=64 && tag>=0);
    uint64_t bytes=NIL_NODE_OVERHEAD+(uint64_t)slots*8;
    if ((uint64_t)slots<NIL_NODE_BUCKETS && nil_node_retired[slots]) {
        NilNode *node=nil_node_retired[slots];
        nil_node_retired[slots]=node->pending;
        nil_node_retired_count[slots]--;
        nil_live+=bytes;
        node->pending=NULL; node->refs=1; node->tag=(uint64_t)tag;
        node->slots=(uint64_t)slots; node->children=(uint64_t)children;
        return node;
    }
    if (nil_should_collect(bytes)) nil_collect();
    if (nil_allocated > NIL_MEMORY_LIMIT-bytes) {
        // Retired capacity is still charged; reclaim it before failing.
        nil_node_flush();
        if (nil_allocated > NIL_MEMORY_LIMIT-bytes) nil_fail(6,start,end);
    }
    NilNode *node=nil_heap_malloc(bytes);
    if (!node) nil_fail(6,start,end);
    nil_allocated+=bytes;
    /* Charged live as well: nodes are never swept, so counting them as
       garbage would make the collector spin trying to recover them. */
    nil_live+=bytes;
    node->pending=NULL; node->refs=1; node->tag=(uint64_t)tag;
    node->slots=(uint64_t)slots; node->children=(uint64_t)children;
    return node;
}
void *nil_node_retain(void *p) {
    if (nil_node_boxed(p)) ((NilNode*)p)->refs++;
    return p;
}
/* Iterative: a 10^6-node drop allocates nothing and does not recurse. */
void nil_node_release(void *p) {
    if (!nil_node_boxed(p)) return;
    NilNode *node=p;
    assert(node->refs>0);
    if (--node->refs) return;
    node->pending=NULL;
    for (NilNode *work=node; work; ) {
        NilNode *current=work; work=current->pending;
        for (uint64_t mask=current->children; mask; mask&=mask-1) {
            void *child=(void*)(uintptr_t)current->payload[__builtin_ctzll(mask)];
            if (!nil_node_boxed(child)) continue;
            NilNode *boxed=child;
            assert(boxed->refs>0);
            if (--boxed->refs==0) { boxed->pending=work; work=boxed; }
        }
        nil_live-=nil_node_bytes(current);
        nil_node_retire(current);
    }
}
/* Transfers `value` into the frame slot and releases whatever the slot held.
   Storing first keeps the new value reachable while the old one drops, which
   matters when the old value is the new one's child. */
void nil_node_slot_store(void **slot,void *value) {
    void *previous=*slot; *slot=value; nil_node_release(previous);
}
void nil_node_frame_release(void **slots,int64_t count) {
    for (int64_t i=0;i<count;i++) { nil_node_release(slots[i]); slots[i]=NULL; }
}
int64_t nil_node_tag(void *p) {
    if (!nil_node_boxed(p)) return (int64_t)((uintptr_t)p>>3);
    return (int64_t)((NilNode*)p)->tag;
}
void nil_node_set(void *p,int64_t slot,int64_t value) {
    NilNode *node=p;
    assert(nil_node_boxed(p) && slot>=0 && (uint64_t)slot<node->slots);
    node->payload[slot]=value;
}
int64_t nil_node_get(void *p,int64_t slot) {
    const NilNode *node=p;
    assert(nil_node_boxed(p) && slot>=0 && (uint64_t)slot<node->slots);
    return node->payload[slot];
}
static void nil_release(void) {
    while(nil_allocations) { NilSequence *next=nil_allocations->next; free(nil_allocations); nil_allocations=next; }
    nil_alloc_count=0; nil_collect_floor=0;
    nil_node_flush();
    nil_allocated=0; nil_live=0; nil_random_ready=false;
}
void *nil_make(int64_t length,int64_t fill,int64_t width,uint64_t start,uint64_t end) {
    if(length<0) nil_fail(16,start,end);
    if(width==1 && (fill<0 || fill>255)) nil_fail(7,start,end);
    NilSequence *value=nil_allocate(length,(uint64_t)width,start,end);
    if(width==1) memset(value->data,(unsigned char)fill,(size_t)length);
    else for(int64_t i=0;i<length;i++) ((int64_t*)value->data)[i]=fill;
    return value;
}
/* Adjacent singleton RHS stays on the activation stack and cannot escape.
   It creates no arena reservation. Bounds/byte validation is unchanged. */
__attribute__((always_inline)) void *nil_make_temporary(NilSequence *value,int64_t fill,uint64_t width,uint64_t start,uint64_t end) {
    if(width==1 && (fill<0 || fill>255)) nil_fail(7,start,end);
    value->next=NULL; value->length=1; value->width=width; value->roots=1; value->capacity=1;
    if(width==1) value->data[0]=(unsigned char)fill;
    else ((int64_t*)value->data)[0]=fill;
    return value;
}
void *nil_literal(const void *bytes,int64_t length,uint64_t start,uint64_t end) {
    NilSequence *value=nil_allocate(length,1,start,end);
    if(length) memcpy(value->data,bytes,(size_t)length); return value;
}
/* Only invoked after a complete validated-provider scan proof. No allocation,
   roots, effects or traps: length guard proves both payload ranges accessible. */
__attribute__((always_inline)) bool nil_bulk_compare(const NilSequence *a,const NilSequence *b) {
    if(a->length!=b->length) return false;
    return a->length==0 || memcmp(a->data,b->data,(size_t)a->length*a->width)==0;
}
__attribute__((always_inline)) int64_t nil_length(const NilSequence *value) { return value->length; }
__attribute__((always_inline)) int64_t nil_get(const NilSequence *value,int64_t index,uint64_t start,uint64_t end) {
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    return value->width==1 ? value->data[index] : ((const int64_t*)value->data)[index];
}
void *nil_set(const NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end) {
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
    NilSequence *copy=nil_allocate_capacity(value->length,value->capacity,value->width,start,end);
    memcpy(copy->data,value->data,(size_t)value->length*value->width);
    nil_copy_hint("replace",(uint64_t)value->length*value->width,1,value);
    if(value->width==1) copy->data[index]=(unsigned char)replacement;
    else ((int64_t*)copy->data)[index]=replacement;
    return copy;
}
/* Static chain proof plus live-root uniqueness protects immutable aliases.
   An update that allocates nothing needs no memory admission. */
__attribute__((always_inline)) void *nil_set_unique_impl(NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end,bool observed) {
    if(!observed) nil_roots_sync();
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
    if(value->roots==0) abort();
    if(value->roots!=1) return nil_set(value,index,replacement,start,end);
    if(value->width==1) value->data[index]=(unsigned char)replacement;
    else ((int64_t*)value->data)[index]=replacement;
    return value;
}
/* Private checked path: the emitter maintains an already-observed frame. */
__attribute__((always_inline)) void *nil_set_unique_observed(NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end) {
    return nil_set_unique_impl(value,index,replacement,start,end,true);
}
__attribute__((always_inline)) void *nil_set_unique(NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end) {
    return nil_set_unique_impl(value,index,replacement,start,end,false);
}
/* A scalar snapshot may move only after every original replacement check and
   before its write. The emitter proves there is no intervening observable work. */
__attribute__((always_inline)) void *nil_set_unique_capture(NilSequence *value,int64_t index,int64_t replacement,int64_t *original,uint64_t start,uint64_t end) {
    nil_roots_sync();
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
    *original=value->width==1 ? value->data[index] : ((int64_t*)value->data)[index];
    return nil_set_unique(value,index,replacement,start,end);
}
/* Guarded scalar replay: every original update check precedes the nontrapping
   probe. No root or managed-allocation observation occurs before its write. */
__attribute__((always_inline)) void nil_set_replay_check(NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end) {
    nil_roots_sync();
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
}
__attribute__((always_inline)) void *nil_set_replay(NilSequence *value,int64_t index,int64_t replacement,bool valid,uint64_t start,uint64_t end) {
    if(!valid) return nil_set(value,index,replacement,start,end);
    return nil_set_unique(value,index,replacement,start,end);
}
static uint64_t nil_concat_capacity(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    if(NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/a->width;
    uint64_t needed=(uint64_t)a->length+(uint64_t)b->length;
    if(needed>maximum) nil_fail(6,start,end);
    if(needed<=a->capacity) return a->capacity;
    uint64_t grown=a->capacity>maximum/2 ? maximum : a->capacity*2;
    return needed>grown ? needed : grown;
}
void *nil_concat(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t capacity=nil_concat_capacity(a,b,start,end);
    NilSequence *copy=nil_allocate_capacity(a->length+b->length,capacity,a->width,start,end);
    memcpy(copy->data,a->data,(size_t)a->length*a->width);
    nil_copy_hint("concat",(uint64_t)a->length*a->width,1,a);
    memcpy(copy->data+(size_t)a->length*a->width,b->data,(size_t)b->length*b->width);
    nil_copy_hint("concat_right",(uint64_t)b->length*b->width,2,b);
    return copy;
}
/* Last-use admission is static; active roots and a distinct RHS discharge all
   remaining aliases. Both operands must be rooted across collection; a stack
   singleton has a synthetic root and is not an arena allocation. Growing
   relocates only this one dead operand's root slot. */
__attribute__((always_inline)) void *nil_concat_unique(NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    nil_roots_sync();
    uint64_t capacity=nil_concat_capacity(a,b,start,end);
    uint64_t bytes=capacity*a->width;
    if(a->roots==0 || b->roots==0) abort();
    if(a->roots!=1 || a==b) return nil_concat(a,b,start,end);
    if(capacity!=a->capacity) {
        nil_collect();
        NilSequence **link=&nil_allocations;
        while(*link && *link!=a) link=&(*link)->next;
        if(!*link) abort();
        NilRootRef root=nil_roots_find(a);
        uint64_t delta=(capacity-a->capacity)*a->width;
        if(delta>NIL_MEMORY_LIMIT || nil_allocated>NIL_MEMORY_LIMIT-delta) nil_fail(6,start,end);
        NilSequence *grown=nil_heap_realloc(a,sizeof(*a)+(size_t)bytes);
        if(!grown) nil_fail(6,start,end);
        grown->capacity=capacity; *link=grown; *root.slot=grown; *root.mirror=grown;
        nil_live+=delta; nil_allocated+=delta; a=grown;
    }
    memcpy(a->data+(size_t)a->length*a->width,b->data,(size_t)b->length*b->width);
    nil_copy_hint("append_right",(uint64_t)b->length*b->width,2,b);
    a->length+=b->length;
    return a;
}
void *nil_slice(const NilSequence *value,int64_t offset,int64_t length,uint64_t start,uint64_t end) {
    if(offset<0 || length<0 || offset>value->length || length>value->length-offset) nil_fail(4,start,end);
    NilSequence *copy=nil_allocate(length,value->width,start,end);
    if(length) memcpy(copy->data,value->data+(size_t)offset*value->width,(size_t)length*value->width);
    nil_copy_hint("slice",(uint64_t)length*value->width,1,value);
    return copy;
}
/* Flat sequences only: record buffers carry child edges and use a separate path.
   Last-use is a compiler hint; synchronized runtime roots remain authoritative. */
void *nil_slice_unique(NilSequence *value,int64_t offset,int64_t length,uint64_t start,uint64_t end) {
    nil_roots_sync();
    if(offset<0 || length<0 || offset>value->length || length>value->length-offset) nil_fail(4,start,end);
    if(value->roots==0) abort();
    if(value->roots!=1) return nil_slice(value,offset,length,start,end);
    if(length && offset) memmove(value->data,value->data+(size_t)offset*value->width,(size_t)length*value->width);
    value->length=length;
    return value;
}
/* ADR 052 part 1b: join a numeric sequence into one row. The exact size is
   computed first and the buffer filled once, so this replaces the quadratic
   per-element !concat chain with a single allocation. */
static int64_t nil_decimal_width(int64_t value) {
    int64_t width=value<0?1:0;
    uint64_t magnitude=value<0?(uint64_t)(-(value+1))+1:(uint64_t)value;
    do { width++; magnitude/=10; } while(magnitude);
    return width;
}
static int64_t nil_write_decimal(unsigned char *out,int64_t value) {
    int64_t width=nil_decimal_width(value);
    uint64_t magnitude=value<0?(uint64_t)(-(value+1))+1:(uint64_t)value;
    int64_t i=width;
    do { out[--i]=(unsigned char)('0'+magnitude%10); magnitude/=10; } while(magnitude);
    if(value<0) out[0]='-';
    return width;
}
static void *nil_format_row_impl(const NilSequence *values,const NilSequence *separator,const NilSequence *terminator,uint64_t start,uint64_t end) {
    const int64_t *items=(const int64_t*)(const void*)values->data;
    int64_t size=terminator?terminator->length:0;
    for(int64_t i=0;i<values->length;i++) size+=nil_decimal_width(items[i]);
    if(values->length>1) size+=separator->length*(values->length-1);
    NilSequence *out=nil_allocate(size,1,start,end);
    int64_t offset=0;
    for(int64_t i=0;i<values->length;i++) {
        if(i) { memcpy(out->data+offset,separator->data,(size_t)separator->length); offset+=separator->length; }
        offset+=nil_write_decimal(out->data+offset,items[i]);
    }
    if(terminator&&terminator->length) memcpy(out->data+offset,terminator->data,(size_t)terminator->length);
    return out;
}
/* Arity is part of the name, so a two-argument call never reads an absent
   terminator pointer. */
void *nil_format_row(const NilSequence *values,const NilSequence *separator,uint64_t start,uint64_t end) {
    return nil_format_row_impl(values,separator,NULL,start,end);
}
void *nil_format_row_term(const NilSequence *values,const NilSequence *separator,const NilSequence *terminator,uint64_t start,uint64_t end) {
    return nil_format_row_impl(values,separator,terminator,start,end);
}
void *nil_format(int64_t value,uint64_t start,uint64_t end) {
    char text[32]; int n=snprintf(text,sizeof(text),"%" PRId64,value);
    return nil_literal(text,n,start,end);
}
static int64_t nil_decimal(const unsigned char *data,int64_t length,uint64_t start,uint64_t end) {
    if(length<1 || length>20) nil_fail(9,start,end);
    char text[32],canonical[32],*tail; memcpy(text,data,(size_t)length); text[length]=0;
    errno=0; intmax_t number=strtoimax(text,&tail,10);
    if(errno==ERANGE || tail!=text+length || number<INT64_MIN || number>INT64_MAX) nil_fail(9,start,end);
    int n=snprintf(canonical,sizeof(canonical),"%" PRId64,(int64_t)number);
    if(n!=length || memcmp(canonical,text,(size_t)n)) nil_fail(9,start,end);
    return (int64_t)number;
}
int64_t nil_parse(const NilSequence *value,uint64_t start,uint64_t end) {
    return nil_decimal(value->data,value->length,start,end);
}

__attribute__((always_inline)) int64_t nil_find(const NilSequence *value,int64_t needle,int64_t offset,uint64_t start,uint64_t end) {
    if(offset<0 || offset>value->length) nil_fail(4,start,end);
    if(value->width==1) {
        if(needle<0 || needle>255) nil_fail(7,start,end);
        const unsigned char *found=memchr(value->data+offset,(unsigned char)needle,(size_t)(value->length-offset));
        return found ? (int64_t)(found-value->data) : value->length;
    }
    const int64_t *data=(const int64_t*)value->data;
    for(int64_t i=offset;i<value->length;i++) if(data[i]==needle) return i;
    return value->length;
}
/* ADR 052 part 1a. Alternating offset and length into the input, so a field is
   recovered with the existing slice and nothing is copied here. Empty fields
   are kept, so the count is always separators + 1; !parsebuf skips them
   because its fields are numbers. */
void *nil_split(const NilSequence *text,const NilSequence *delimiters,uint64_t start,uint64_t end) {
    bool separators[256]={false};
    for(int64_t i=0;i<delimiters->length;i++) separators[delimiters->data[i]]=true;
    int64_t fields=1;
    for(int64_t i=0;i<text->length;i++) if(separators[text->data[i]]) fields++;
    NilSequence *result=nil_allocate(fields*2,8,start,end);
    int64_t *out=(int64_t*)(void*)result->data;
    int64_t index=0,offset=0;
    for(int64_t i=0;i<text->length;i++) if(separators[text->data[i]]) {
        out[index++]=offset; out[index++]=i-offset; offset=i+1;
    }
    out[index++]=offset; out[index++]=text->length-offset;
    return result;
}
void *nil_parsebuf(const NilSequence *text,const NilSequence *delimiters,uint64_t start,uint64_t end) {
    bool separators[256]={false};
    for(int64_t i=0;i<delimiters->length;i++) separators[delimiters->data[i]]=true;
    /* Empty fields are skipped (ADR 052 step 0), so a run of separators
       behaves like one and leading or trailing separators contribute nothing. */
    int64_t count=0,run=0;
    for(int64_t i=0;i<text->length;i++) {
        if(separators[text->data[i]]) { if(run) count++; run=0; } else run++;
    }
    if(run) count++;
    /* Allocate output before conversion; operands remain rooted. Resource failure
       placement is an implementation detail, not a deterministic quota rule. */
    NilSequence *result=nil_allocate(count,8,start,end);
    int64_t offset=0,index=0;
    for(int64_t i=0;i<text->length;i++) if(separators[text->data[i]]) {
        if(i>offset) ((int64_t*)result->data)[index++]=nil_decimal(text->data+offset,i-offset,start,end);
        offset=i+1;
    }
    if(offset<text->length) ((int64_t*)result->data)[index++]=nil_decimal(text->data+offset,text->length-offset,start,end);
    return result;
}

static char *nil_path(const NilSequence *value,uint64_t start,uint64_t end) {
    if(memchr(value->data,0,(size_t)value->length)) nil_fail(10,start,end);
    char *name=malloc((size_t)value->length+1); if(!name) nil_fail(6,start,end);
    memcpy(name,value->data,(size_t)value->length); name[value->length]=0; return name;
}
/* Explicit process policy for the private application driver; not a sandbox. */
static void nil_host_permission(uint64_t start,uint64_t end) {
    const char *denied=getenv("NIL_DENY_HOST_IO");
    if(denied && strcmp(denied,"1")==0) nil_fail(11,start,end);
}
void *nil_read(const NilSequence *path,uint64_t start,uint64_t end) {
    nil_collect();
    char *name=nil_path(path,start,end); nil_host_permission(start,end); FILE *file=fopen(name,"rb"); free(name);
    if(!file) nil_fail(8,start,end);
    /* st_size is a capacity hint, never an error oracle: truncation, growth and
       short/error reads are decided by the bytes actually obtained. Read one
       byte beyond the safety budget: fail fast on an excess prefix rather than
       consume a stream that subsequently fails. E013 placement is environmental. */
    size_t available=NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD || nil_allocated>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD ? 0 :
        (size_t)(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD-nil_allocated);
    size_t maximum=available+1,capacity=maximum<65536 ? maximum : 65536,length=0;
    struct stat info;
    if(fstat(fileno(file),&info)==0 && S_ISREG(info.st_mode) && info.st_size>=0) {
        uint64_t hint=(uint64_t)info.st_size;
        capacity=hint>=maximum ? maximum : (size_t)hint+1;
    }
    /* Unpublished host scratch becomes the arena payload only after successful
       I/O; admission retains the original I/O-before-final-allocation ordering. */
    NilSequence *value=nil_heap_malloc(sizeof(*value)+capacity); if(!value) nil_fail(6,start,end);
    for(;;) {
        if(length==capacity) {
            size_t next_capacity=capacity>maximum/2 ? maximum : capacity*2;
            NilSequence *next=nil_heap_realloc(value,sizeof(*value)+next_capacity);
            if(!next) nil_fail(6,start,end);
            value=next; capacity=next_capacity;
        }
        size_t received=fread(value->data+length,1,capacity-length,file);
        length+=received;
        if(length>available) nil_fail(6,start,end);
        if(ferror(file) || feof(file)) break;
        if(!received) { free(value); fclose(file); nil_fail(8,start,end); }
    }
    int failed=ferror(file); if(fclose(file)!=0) failed=1;
    if(failed) nil_fail(8,start,end);
    if(NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD || nil_allocated>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    NilSequence *exact=nil_heap_realloc(value,sizeof(*value)+length);
    if(!exact) nil_fail(6,start,end);
    value=exact;
    nil_allocated+=(uint64_t)length+NIL_SEQUENCE_OVERHEAD;
    value->capacity=length;
    value->length=(int64_t)length; value->width=1; value->roots=0;
    value->next=nil_allocations; nil_allocations=value;
    return value;
}
int64_t nil_write(const NilSequence *path,const NilSequence *data,uint64_t start,uint64_t end) {
    char *name=nil_path(path,start,end); nil_host_permission(start,end); FILE *file=fopen(name,"wb"); free(name);
    if(!file) nil_fail(8,start,end);
    size_t written=fwrite(data->data,1,(size_t)data->length,file);
    int failed=written!=(size_t)data->length; if(fclose(file)!=0) failed=1;
    if(failed) nil_fail(8,start,end); return data->length;
}
int64_t nil_out(const NilSequence *data,uint64_t start,uint64_t end) {
    nil_host_permission(start,end);
    if(fwrite(data->data,1,(size_t)data->length,stdout)!=(size_t)data->length || fflush(stdout)!=0) nil_fail(8,start,end);
    return data->length;
}
static NilSequence *nil_buffer_argument(const char *text) {
    const char *p=text; int64_t count=0;
    if(*p++!='[') { fputs("E010 buffer argument must be [I64,...]\n",stderr); exit(2); }
    if(*p!=']') {
        for(;;) {
            char *tail; errno=0; intmax_t v=strtoimax(p,&tail,10);
            if(tail==p || errno==ERANGE || v<INT64_MIN || v>INT64_MAX || (*p!='-' && (*p<'0' || *p>'9'))) { fputs("E010 invalid buffer element\n",stderr); exit(2); }
            count++; p=tail;
            if(*p==']') break;
            if(*p++!=',') { fputs("E010 invalid buffer argument\n",stderr); exit(2); }
        }
    }
    if(*p++!=']' || *p) { fputs("E010 invalid buffer argument\n",stderr); exit(2); }
    NilSequence *result=nil_make(count,0,8,UINT64_MAX,UINT64_MAX); p=text+1;
    for(int64_t i=0;i<count;i++) { char *tail; ((int64_t*)result->data)[i]=(int64_t)strtoimax(p,&tail,10); p=tail+1; }
    return result;
}

/* Maps own their entire byte arena; they have no child allocation edges. The
   common header/root charge remains capacity bytes + 40. No field ABI is public. */
typedef struct { uint64_t entries, bytes, used; } NilMapMeta;
typedef struct { uint64_t hash, key, key_len, value, value_len; int64_t number; } NilMapEntry;
_Static_assert(sizeof(NilMapMeta)==24 && sizeof(NilMapEntry)==48, "map payload ABI mismatch");
static NilMapMeta *nil_map_meta(const NilSequence *m) { return (NilMapMeta*)(void*)m->data; }
static NilMapEntry *nil_map_entries(const NilSequence *m) { return (NilMapEntry*)(void*)(m->data+24); }
static uint64_t *nil_map_buckets(const NilSequence *m) { return (uint64_t*)(void*)(m->data+24+48*nil_map_meta(m)->entries); }
static unsigned char *nil_map_bytes(const NilSequence *m) { return (unsigned char*)(void*)(m->data+24+64*nil_map_meta(m)->entries); }
static uint64_t nil_map_hash(const unsigned char *key,int64_t key_len) {
    uint64_t h=UINT64_C(14695981039346656037);
    for(int64_t i=0;i<key_len;i++) h=(h^key[i])*UINT64_C(1099511628211);
    return h;
}
/* Insertion order lives in entries, never in hash-table traversal order. */
static uint64_t nil_map_find(const NilSequence *m,const unsigned char *key,int64_t key_len,uint64_t hash,uint64_t *bucket) {
    uint64_t mask=2*nil_map_meta(m)->entries-1, slot=hash&mask;
    uint64_t *table=nil_map_buckets(m); NilMapEntry *entries=nil_map_entries(m);
    while(table[slot]) {
        uint64_t i=table[slot]-1; NilMapEntry *e=&entries[i];
        if(e->hash==hash && e->key_len==(uint64_t)key_len && !memcmp(nil_map_bytes(m)+e->key,key,e->key_len)) { *bucket=slot;return i; }
        slot=(slot+1)&mask;
    }
    *bucket=slot;return UINT64_MAX;
}
static void nil_map_rehash(NilSequence *m) {
    uint64_t *table=nil_map_buckets(m), mask=2*nil_map_meta(m)->entries-1;
    memset(table,0,2*nil_map_meta(m)->entries*sizeof(*table));
    for(int64_t i=0;i<m->length;i++) {
        uint64_t slot=nil_map_entries(m)[i].hash&mask;
        while(table[slot]) slot=(slot+1)&mask;
        table[slot]=(uint64_t)i+1;
    }
}
void *nil_map(uint64_t start,uint64_t end) {
    NilSequence *m=nil_allocate_capacity(0,24+64*4+16,1,start,end);
    *nil_map_meta(m)=(NilMapMeta){4,16,0}; nil_map_rehash(m); return m;
}
bool nil_map_has(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) {
    (void)start;(void)end;uint64_t slot;return nil_map_find(m,key->data,key->length,nil_map_hash(key->data,key->length),&slot)!=UINT64_MAX;
}
/* Forward declarations, so the integer map can sit beside the other keyed
   operations without depending on definition order. */
bool nil_set_has(const NilSequence *m,int64_t key,uint64_t start,uint64_t end);
void *nil_set_remove(NilSequence *m,int64_t key,uint64_t start,uint64_t end);
static void nil_set_key(int64_t key,unsigned char out[8]);
static const NilMapEntry *nil_map_get_entry_raw(const NilSequence *m,const unsigned char *key,int64_t key_len,uint64_t start,uint64_t end);
static NilSequence *nil_map_update_raw(NilSequence *m,const unsigned char *key,int64_t key_len,int64_t number,const unsigned char *value,uint64_t value_len,bool insert,bool unique,uint64_t start,uint64_t end);

/* Integer-keyed map (ADR 051 part 2). It shares the set's key encoding, so the
   emitter passes a plain i64 and nothing is allocated per call. Values are
   i64, held in the entry's number field exactly as for the byte-keyed i64 map,
   and iteration order is insertion order. */
bool nil_imap_has(const NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    return nil_set_has(m,key,start,end);
}
int64_t nil_imap_get(const NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    unsigned char bytes[8]; nil_set_key(key,bytes);
    return nil_map_get_entry_raw(m,bytes,8,start,end)->number;
}
/* Iteration hands back the key as an i64, not as eight bytes. */
int64_t nil_imap_key(const NilSequence *m,int64_t index,uint64_t start,uint64_t end) {
    if(index<0 || index>=m->length) nil_fail(4,start,end);
    const NilMapEntry *e=&nil_map_entries(m)[index];
    const unsigned char *k=nil_map_bytes(m)+e->key;
    uint64_t bits=0;
    for(int i=0;i<8;i++) bits=(bits<<8)|k[i];
    return (int64_t)bits;
}
static void *nil_imap_put_impl(NilSequence *m,int64_t key,int64_t value,bool unique,uint64_t start,uint64_t end) {
    unsigned char bytes[8]; nil_set_key(key,bytes);
    return nil_map_update_raw(m,bytes,8,value,NULL,0,false,unique,start,end);
}
void *nil_imap_put(NilSequence *m,int64_t key,int64_t value,uint64_t start,uint64_t end) {
    return nil_imap_put_impl(m,key,value,false,start,end);
}
void *nil_imap_put_unique(NilSequence *m,int64_t key,int64_t value,uint64_t start,uint64_t end) {
    return nil_imap_put_impl(m,key,value,true,start,end);
}
void *nil_imap_remove(NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    return nil_set_remove(m,key,start,end);
}
int64_t nil_map_size(const NilSequence *m,uint64_t start,uint64_t end) { (void)start;(void)end;return m->length; }
static const NilMapEntry *nil_map_get_entry_raw(const NilSequence *m,const unsigned char *key,int64_t key_len,uint64_t start,uint64_t end) {
    uint64_t slot,i=nil_map_find(m,key,key_len,nil_map_hash(key,key_len),&slot);
    if(i==UINT64_MAX) nil_fail(12,start,end);
    return &nil_map_entries(m)[i];
}
static const NilMapEntry *nil_map_get_entry(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) {
    return nil_map_get_entry_raw(m,key->data,key->length,start,end);
}
int64_t nil_map_get_int(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) { return nil_map_get_entry(m,key,start,end)->number; }
void *nil_map_get_bytes(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) {
    const NilMapEntry *e=nil_map_get_entry(m,key,start,end);
    NilSequence *out=nil_literal(nil_map_bytes(m)+e->value,(int64_t)e->value_len,start,end);
    nil_copy_hint("map_value",e->value_len,2,m);return out;
}
void *nil_map_key(const NilSequence *m,int64_t index,uint64_t start,uint64_t end) {
    if(index<0 || index>=m->length) nil_fail(4,start,end);
    const NilMapEntry *e=&nil_map_entries(m)[index];
    NilSequence *out=nil_literal(nil_map_bytes(m)+e->key,(int64_t)e->key_len,start,end);
    nil_copy_hint("map_key",e->key_len,2,m);return out;
}
/* Last-use + unique live root allows writes; all other cases copy. */
static NilSequence *nil_map_update_raw(NilSequence *m,const unsigned char *key,int64_t key_len,int64_t number,const unsigned char *value,uint64_t value_len,bool insert,bool unique,uint64_t start,uint64_t end) {
    uint64_t slot, hash=nil_map_hash(key,key_len), i=nil_map_find(m,key,key_len,hash,&slot);
    if(insert && i!=UINT64_MAX) nil_fail(13,start,end);
    uint64_t old_len=i==UINT64_MAX?0:nil_map_entries(m)[i].value_len;
    uint64_t used=nil_map_meta(m)->used-old_len+value_len+(i==UINT64_MAX?(uint64_t)key_len:0);
    uint64_t count=(uint64_t)m->length+(i==UINT64_MAX);
    uint64_t ec=nil_map_meta(m)->entries, bc=nil_map_meta(m)->bytes;
    while(ec<count) ec*=2;
    while(bc<used) bc*=2;
    uint64_t capacity=24+64*ec+bc;
    nil_roots_sync();
    if(!m->roots) abort();
    bool reusable=unique && m->roots==1 && capacity==m->capacity && (i==UINT64_MAX || old_len==value_len);
    NilSequence *out=m;
    if(!reusable) {
        out=nil_allocate_capacity(m->length,capacity,1,start,end);
        *nil_map_meta(out)=(NilMapMeta){ec,bc,0};
        for(int64_t j=0;j<m->length;j++) {
            NilMapEntry e=nil_map_entries(m)[j]; unsigned char *dst=nil_map_bytes(out); const unsigned char *src=nil_map_bytes(m);
            uint64_t offset=nil_map_meta(out)->used;
            if(e.key_len) memcpy(dst+offset,src+e.key,e.key_len);
            e.key=offset; offset+=e.key_len;
            uint64_t next_len=(uint64_t)j==i?value_len:e.value_len;
            if((uint64_t)j==i) { if(value_len) memcpy(dst+offset,value,value_len); e.number=number; }
            else if(e.value_len) memcpy(dst+offset,src+e.value,e.value_len);
            e.value=offset;e.value_len=next_len;nil_map_meta(out)->used=offset+next_len;
            nil_map_entries(out)[j]=e;
        }
#if NIL_EXPLAIN_COPIES
        uint64_t copied=(uint64_t)m->length*sizeof(NilMapEntry);
        for(int64_t j=0;j<m->length;j++) {
            const NilMapEntry *e=&nil_map_entries(m)[j];
            copied+=e->key_len;if((uint64_t)j!=i)copied+=e->value_len;
        }
        if(copied) nil_copy_hint("map_rebuild",copied,1,m);
#endif
        nil_map_rehash(out);
        if(i!=UINT64_MAX) { nil_copy_hint("map_update_value",value_len,2,NULL);return out; }
        (void)nil_map_find(out,key,key_len,hash,&slot);
    }
    if(i==UINT64_MAX) {
        i=(uint64_t)out->length++;
        uint64_t offset=nil_map_meta(out)->used;
        if(key_len) memcpy(nil_map_bytes(out)+offset,key,(size_t)key_len);
        /* Role 2 never reads the source — nil_hint_retaining is guarded by
           role==1 — and since the raw-key refactor there is no NilSequence for
           the key to point at, so this passes NULL rather than a byte pointer
           through a NilSequence parameter. */
        nil_copy_hint("map_insert_key",(uint64_t)key_len,2,NULL);
        nil_map_entries(out)[i]=(NilMapEntry){hash,offset,(uint64_t)key_len,offset+(uint64_t)key_len,value_len,number};
        nil_map_buckets(out)[slot]=i+1;nil_map_meta(out)->used=used;
    }
    NilMapEntry *e=&nil_map_entries(out)[i];e->number=number;
    if(value_len) memcpy(nil_map_bytes(out)+e->value,value,value_len);
    nil_copy_hint("map_update_value",value_len,2,NULL);
    return out;
}
#define NIL_MAP_UPDATE(name,insert,unique) \
void *nil_map_##name##_int(NilSequence *m,const NilSequence *key,int64_t value,uint64_t start,uint64_t end) { return nil_map_update_raw(m,key->data,key->length,value,NULL,0,insert,unique,start,end); } \
void *nil_map_##name##_bytes(NilSequence *m,const NilSequence *key,const NilSequence *value,uint64_t start,uint64_t end) { return nil_map_update_raw(m,key->data,key->length,0,value->data,(uint64_t)value->length,insert,unique,start,end); }
NIL_MAP_UPDATE(insert,true,false)
NIL_MAP_UPDATE(insert_unique,true,true)
NIL_MAP_UPDATE(put,false,false)
NIL_MAP_UPDATE(put_unique,false,true)
/* Integer-keyed set (ADR 051). The eight big-endian bytes are formed on the C
   stack, so membership and insertion allocate nothing per call and the emitter
   passes a plain i64. Correctness first: this reuses the byte-keyed table
   rather than adding an integer hash, which is an optimization to justify by
   measurement. */
static void nil_set_key(int64_t key,unsigned char out[8]) {
    uint64_t bits=(uint64_t)key;
    for(int i=0;i<8;i++) out[i]=(unsigned char)(bits>>(56-8*i));
}
bool nil_set_has(const NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    (void)start;(void)end;
    unsigned char bytes[8]; nil_set_key(key,bytes);
    uint64_t slot;
    return nil_map_find(m,bytes,8,nil_map_hash(bytes,8),&slot)!=UINT64_MAX;
}
static void *nil_set_add_impl(NilSequence *m,int64_t key,bool unique,uint64_t start,uint64_t end) {
    unsigned char bytes[8]; nil_set_key(key,bytes);
    return nil_map_update_raw(m,bytes,8,0,NULL,0,false,unique,start,end);
}
/* Uniqueness is chosen by the emitter from last use, as for map updates, so it
   is part of the name rather than a runtime flag. */
void *nil_set_add(NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    return nil_set_add_impl(m,key,false,start,end);
}
void *nil_set_add_unique(NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    return nil_set_add_impl(m,key,true,start,end);
}
/* Bulk construction. Iteration order is first occurrence, so a repeated key is
   a no-op, matching the reference. The table is sized for the worst case of
   all-distinct keys and filled in one pass, so this never reallocates and
   never consults ownership: the set being built is not yet a value anyone
   holds, so the per-update root checks do not apply to it. */
void *nil_set_from(const NilSequence *source,uint64_t start,uint64_t end) {
    int64_t n=source->length;
    uint64_t ec=4,bc=16;
    while(ec<(uint64_t)n) ec*=2;
    while(bc<(uint64_t)n*8) bc*=2;
    NilSequence *m=nil_allocate_capacity(0,24+64*ec+bc,1,start,end);
    *nil_map_meta(m)=(NilMapMeta){ec,bc,0};
    nil_map_rehash(m);
    uint64_t mask=2*ec-1;
    for(int64_t i=0;i<n;i++) {
        unsigned char bytes[8];
        nil_set_key(((const int64_t*)(const void*)source->data)[i],bytes);
        uint64_t slot,hash=nil_map_hash(bytes,8);
        if(nil_map_find(m,bytes,8,hash,&slot)!=UINT64_MAX) continue;
        uint64_t offset=nil_map_meta(m)->used;
        memcpy(nil_map_bytes(m)+offset,bytes,8);
        nil_map_entries(m)[m->length]=(NilMapEntry){hash,offset,8,offset+8,0,0};
        nil_map_meta(m)->used=offset+8;
        m->length+=1;
        nil_map_buckets(m)[slot&mask]=(uint64_t)m->length;
    }
    return m;
}
/* Removal preserves insertion order, which is the iteration contract, so the
   entry is excised and the bucket table rebuilt. That is linear in entries;
   membership and insertion stay unaffected. */
void *nil_set_remove(NilSequence *m,int64_t key,uint64_t start,uint64_t end) {
    unsigned char bytes[8]; nil_set_key(key,bytes);
    uint64_t slot, hash=nil_map_hash(bytes,8), found=nil_map_find(m,bytes,8,hash,&slot);
    if(found==UINT64_MAX) return m;
    NilSequence *out=nil_allocate_capacity(m->length-1,m->capacity,1,start,end);
    *nil_map_meta(out)=(NilMapMeta){nil_map_meta(m)->entries,nil_map_meta(m)->bytes,0};
    int64_t written=0;
    for(int64_t j=0;j<m->length;j++) {
        if((uint64_t)j==found) continue;
        NilMapEntry e=nil_map_entries(m)[j];
        uint64_t offset=nil_map_meta(out)->used;
        if(e.key_len) memcpy(nil_map_bytes(out)+offset,nil_map_bytes(m)+e.key,e.key_len);
        e.key=offset; offset+=e.key_len;
        if(e.value_len) memcpy(nil_map_bytes(out)+offset,nil_map_bytes(m)+e.value,e.value_len);
        e.value=offset; offset+=e.value_len;
        nil_map_meta(out)->used=offset;
        nil_map_entries(out)[written++]=e;
    }
    nil_map_rehash(out);
    return out;
}
/* Scalar-record map slots have a specified little-endian encoding, independent
   of target byte order. No pointers/child aliases enter the packed payload. */
void nil_map_get_record(const NilSequence *m,const NilSequence *key,uint64_t *out,uint64_t slots,uint64_t start,uint64_t end) {
    const NilMapEntry *entry=nil_map_get_entry(m,key,start,end);
    if(slots>4096 || entry->value_len!=slots*8) abort();
    const unsigned char *data=nil_map_bytes(m)+entry->value;
    for(uint64_t i=0;i<slots;i++) { uint64_t word=0;for(unsigned b=0;b<8;b++) word|=(uint64_t)data[i*8+b]<<(8*b);out[i]=word; }
    nil_copy_hint("map_record_value",slots*8,2,m);
}
void *nil_map_update_record(NilSequence *m,const NilSequence *key,const uint64_t *value,uint64_t slots,bool insert,bool unique,uint64_t start,uint64_t end) {
    if(slots>4096) abort();
    unsigned char bytes[4096*8];
    for(uint64_t i=0;i<slots;i++) for(unsigned b=0;b<8;b++) bytes[i*8+b]=(unsigned char)(value[i]>>(8*b));
    return nil_map_update_raw(m,key->data,key->length,0,bytes,slots*8,insert,unique,start,end);
}
static void nil_print_hex(const unsigned char *data,uint64_t length) {
    putchar('"');for(uint64_t i=0;i<length;i++) printf("%02x",data[i]);putchar('"');
}
static void nil_map_print(const NilSequence *m,bool bytes) {
    putchar('[');for(int64_t i=0;i<m->length;i++) {
        const NilMapEntry *e=&nil_map_entries(m)[i];if(i) putchar(',');putchar('[');
        nil_print_hex(nil_map_bytes(m)+e->key,e->key_len);putchar(',');
        if(bytes) nil_print_hex(nil_map_bytes(m)+e->value,e->value_len);else printf("%" PRId64,e->number);
        putchar(']');
    } putchar(']');
}

/* Sort has a complete, deterministic comparator. Map key ties cannot occur;
   primitive equal elements have no identity. Constant scratch and heapsort avoid
   n-sized temporary storage outside the live-capacity budget. */
static int nil_byte_order(const unsigned char *a,uint64_t an,const unsigned char *b,uint64_t bn) {
    uint64_t n=an<bn?an:bn;
    int c=n?memcmp(a,b,(size_t)n):0;
    return c ? (c<0?-1:1) : (an>bn)-(an<bn);
}
static int nil_sort_order(const NilSequence *m,uint64_t a,uint64_t b,int64_t order,int64_t kind) {
    if(kind<2) {
        int64_t av=kind==1?m->data[a]:((const int64_t*)m->data)[a];
        int64_t bv=kind==1?m->data[b]:((const int64_t*)m->data)[b];
        int c=(av>bv)-(av<bv); return order ? -c:c;
    }
    const NilMapEntry *av=&nil_map_entries(m)[a],*bv=&nil_map_entries(m)[b];
    const unsigned char *data=nil_map_bytes(m); int c=0;
    if(order) c=kind==2 ? (av->number>bv->number)-(av->number<bv->number) :
        nil_byte_order(data+av->value,av->value_len,data+bv->value,bv->value_len);
    return c?c:nil_byte_order(data+av->key,av->key_len,data+bv->key,bv->key_len);
}
static void nil_sort_swap(NilSequence *m,uint64_t a,uint64_t b,int64_t kind) {
    if(kind>=2) { NilMapEntry *e=nil_map_entries(m),tmp=e[a]; e[a]=e[b];e[b]=tmp; }
    else if(kind==1) { unsigned char tmp=m->data[a];m->data[a]=m->data[b];m->data[b]=tmp; }
    else { int64_t *e=(int64_t*)m->data,tmp=e[a];e[a]=e[b];e[b]=tmp; }
}
static void nil_sort_sift(NilSequence *m,uint64_t root,uint64_t count,int64_t order,int64_t kind) {
    while(root<count/2) {
        uint64_t child=root*2+1;
        if(child+1<count && nil_sort_order(m,child,child+1,order,kind)<0) child++;
        if(nil_sort_order(m,root,child,order,kind)>=0) return;
        nil_sort_swap(m,root,child,kind); root=child;
    }
}
static NilSequence *nil_sort_impl(NilSequence *value,int64_t order,int64_t kind,bool unique,uint64_t start,uint64_t end) {
    if(order<0 || order>1) nil_fail(4,start,end);
    nil_roots_sync();
    if(!unique || value->roots!=1) {
        uint64_t bytes=value->capacity*value->width;
        NilSequence *copy=nil_allocate_capacity(value->length,value->capacity,value->width,start,end);
        memcpy(copy->data,value->data,(size_t)bytes);
        nil_copy_hint("sort",bytes,1,value); value=copy;
    }
    uint64_t n=(uint64_t)value->length;
    for(uint64_t i=n/2;i>0;i--) nil_sort_sift(value,i-1,n,order,kind);
    for(uint64_t end=n;end>1;end--) { nil_sort_swap(value,0,end-1,kind);nil_sort_sift(value,0,end-1,order,kind); }
    if(kind>=2) nil_map_rehash(value);
    return value;
}
void *nil_sort(NilSequence *value,int64_t order,int64_t kind,uint64_t start,uint64_t end) { return nil_sort_impl(value,order,kind,false,start,end); }
void *nil_sort_unique(NilSequence *value,int64_t order,int64_t kind,uint64_t start,uint64_t end) { return nil_sort_impl(value,order,kind,true,start,end); }

/* Acyclic record buffers: descriptor plus packed slot rows, private target layout.
   Descriptor constants are compiler-owned; children have shadow-root edges, not
   a user-visible reference-counting or cyclic collection mechanism. */
static const uint64_t *nil_record_descriptor(const NilSequence *v) {
    const uint64_t *d; memcpy(&d,v->data,sizeof(d)); return d;
}
static unsigned char *nil_record_rows(const NilSequence *v) {
    return (unsigned char*)v->data+(v->width & ~NIL_RECORD_TAG);
}
static void nil_row_roots(const uint64_t *desc,const void *row,bool retain) {
    const uint64_t *words=row;
    for(uint64_t i=0;i<desc[1];i++) {
        NilSequence *child=(NilSequence*)(uintptr_t)words[desc[2+i]];
        if(retain) nil_retain(child);else nil_drop(child);
    }
}
static void nil_child_roots(NilSequence *v,bool retain) {
    if(!(v->width & NIL_RECORD_TAG)) return;
    const uint64_t *desc=nil_record_descriptor(v);
    if(!desc[1]) return;
    uint64_t width=v->width & ~NIL_RECORD_TAG;
    unsigned char *rows=nil_record_rows(v);
    for(int64_t i=0;i<v->length;i++) nil_row_roots(desc,rows+(uint64_t)i*width,retain);
}
#if NIL_EXPLAIN_COPIES
static bool nil_hint_reaches(const NilSequence *parent,const NilSequence *source,unsigned depth,unsigned *budget) {
    if(!*budget) return false;
    --*budget;
    if(parent==source) return true;
    if(!parent || depth>=128 || !(parent->width & NIL_RECORD_TAG)) return false;
    const uint64_t *desc=nil_record_descriptor(parent);
    if(!desc[1]) return false;
    uint64_t width=parent->width & ~NIL_RECORD_TAG;
    const unsigned char *rows=nil_record_rows(parent);
    for(int64_t i=0;i<parent->length;i++) {
        if(!*budget) return false;
        const uint64_t *row=(const uint64_t*)(const void*)(rows+(uint64_t)i*width);
        for(uint64_t j=0;j<desc[1];j++)
            if(nil_hint_reaches((const NilSequence*)(uintptr_t)row[desc[2+j]],source,depth+1,budget)) return true;
    }
    return false;
}
static bool nil_hint_retaining(const NilSequence *source,uint64_t *start,uint64_t *end) {
    if(!source) return false;
    unsigned budget=4096;
    for(NilHintRoots *h=nil_hint_roots;h;h=h->previous)
        for(uint64_t i=0;i<h->frame->count;i++) {
            uint64_t s=h->spans[i][0],e=h->spans[i][1];
            /* Ignore the current instruction itself and metadata with no future use. */
            if(s==UINT64_MAX || e==0 || (s==nil_hint_start && e==nil_hint_end)) continue;
            if(nil_hint_reaches(h->frame->slots[i],source,0,&budget)) { *start=s;*end=e;return true; }
        }
    return false;
}
#endif
static NilSequence *nil_record_allocate(int64_t length,uint64_t capacity,const uint64_t *desc,uint64_t start,uint64_t end) {
    if(length<0) nil_fail(16,start,end);
    assert(desc[0] && capacity >= (uint64_t)length);
    uint64_t width=desc[0]*8;
    if(NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD || width>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width-1;
    if(capacity>maximum) nil_fail(6,start,end);
    uint64_t bytes=(capacity+1)*width+NIL_SEQUENCE_OVERHEAD;
    if(nil_should_collect(bytes)) nil_collect();
    if(nil_allocated>NIL_MEMORY_LIMIT-bytes) nil_fail(6,start,end);
    NilSequence *v=nil_heap_malloc((size_t)bytes);if(!v) nil_fail(6,start,end);
    v->next=nil_allocations;nil_allocations=v;v->length=length;v->capacity=capacity;
    v->roots=0;v->width=width|NIL_RECORD_TAG;memcpy(v->data,&desc,sizeof(desc));
    nil_allocated+=bytes;return v;
}
void *nil_record_make(int64_t length,const void *fill,const uint64_t *desc,uint64_t start,uint64_t end) {
    NilSequence *v=nil_record_allocate(length,length<0 ? 0 : (uint64_t)length,desc,start,end);
    uint64_t width=desc[0]*8;
    for(int64_t i=0;i<length;i++) memcpy(nil_record_rows(v)+(uint64_t)i*width,fill,(size_t)width);
    return v;
}
void nil_record_get(const NilSequence *v,int64_t index,void *out,uint64_t start,uint64_t end) {
    if(index<0 || index>=v->length) nil_fail(4,start,end);
    uint64_t width=v->width & ~NIL_RECORD_TAG;
    memcpy(out,nil_record_rows(v)+(uint64_t)index*width,(size_t)width);
    nil_copy_hint("record_get",width,2,v);
}
void *nil_record_set(NilSequence *v,int64_t index,const void *replacement,bool unique,uint64_t start,uint64_t end) {
    if(index<0 || index>=v->length) nil_fail(4,start,end);
    nil_roots_sync();
    uint64_t width=v->width & ~NIL_RECORD_TAG;
    const uint64_t *desc=nil_record_descriptor(v);
    if(!unique || v->roots!=1) {
        NilSequence *copy=nil_record_allocate(v->length,v->capacity,desc,start,end);
        memcpy(nil_record_rows(copy),nil_record_rows(v),(size_t)v->length*width);
        nil_copy_hint("record_set",(uint64_t)v->length*width,1,v);
        memcpy(nil_record_rows(copy)+(uint64_t)index*width,replacement,(size_t)width);
        nil_copy_hint("record_update_value",width,2,NULL);
        return copy;
    }
    void *row=nil_record_rows(v)+(uint64_t)index*width;
    /* Retain replacements before releasing old aliases; no collection inside. */
    nil_row_roots(desc,replacement,true);nil_row_roots(desc,row,false);
    memcpy(row,replacement,(size_t)width);
    nil_copy_hint("record_update_value",width,2,NULL);return v;
}
static uint64_t nil_record_capacity(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t width=a->width & ~NIL_RECORD_TAG;
    assert(width);
    if(NIL_MEMORY_LIMIT<NIL_SEQUENCE_OVERHEAD || width>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width-1;
    uint64_t needed=(uint64_t)a->length+(uint64_t)b->length;
    if(needed>maximum) nil_fail(6,start,end);
    if(needed<=a->capacity)return a->capacity;
    uint64_t grown=a->capacity>maximum/2 ? maximum : a->capacity*2;
    return needed>grown ? needed : grown;
}
void *nil_record_concat(NilSequence *a,const NilSequence *b,bool unique,uint64_t start,uint64_t end) {
    nil_roots_sync();
    uint64_t cap=nil_record_capacity(a,b,start,end),width=a->width & ~NIL_RECORD_TAG;
    uint64_t bytes=NIL_SEQUENCE_OVERHEAD+(cap+1)*width;
    /* RHS rows/children must survive collection before growing the LHS. */
    if(b->roots==0) abort();
    if(!unique || a->roots!=1 || a==b) {
        NilSequence *copy=nil_record_allocate(a->length+b->length,cap,nil_record_descriptor(a),start,end);
        memcpy(nil_record_rows(copy),nil_record_rows(a),(size_t)a->length*width);
        nil_copy_hint("record_concat",(uint64_t)a->length*width,1,a);
        memcpy(nil_record_rows(copy)+(uint64_t)a->length*width,nil_record_rows(b),(size_t)b->length*width);
        nil_copy_hint("record_concat_right",(uint64_t)b->length*width,2,b);
        return copy;
    }
    if(cap!=a->capacity) {
        nil_collect();NilSequence **link=&nil_allocations;
        while(*link && *link!=a)link=&(*link)->next;if(!*link)abort();
        NilRootRef root=nil_roots_find(a);uint64_t delta=(cap-a->capacity)*width;
        if(delta>NIL_MEMORY_LIMIT || nil_allocated>NIL_MEMORY_LIMIT-delta) nil_fail(6,start,end);
        NilSequence *grown=nil_heap_realloc(a,(size_t)bytes);if(!grown)nil_fail(6,start,end);
        grown->capacity=cap;*link=grown;*root.slot=grown;*root.mirror=grown;nil_live+=delta;nil_allocated+=delta;a=grown;
    }
    const uint64_t *desc=nil_record_descriptor(a);
    for(int64_t i=0;i<b->length;i++)nil_row_roots(desc,nil_record_rows(b)+(uint64_t)i*width,true);
    memcpy(nil_record_rows(a)+(uint64_t)a->length*width,nil_record_rows(b),(size_t)b->length*width);
    nil_copy_hint("record_append_right",(uint64_t)b->length*width,2,b);
    a->length+=b->length;return a;
}
void *nil_record_slice(const NilSequence *v,int64_t offset,int64_t length,uint64_t start,uint64_t end) {
    if(offset<0 || length<0 || offset>v->length || length>v->length-offset)nil_fail(4,start,end);
    NilSequence *copy=nil_record_allocate(length,length<0 ? 0 : (uint64_t)length,nil_record_descriptor(v),start,end);
    uint64_t width=v->width & ~NIL_RECORD_TAG;
    memcpy(nil_record_rows(copy),nil_record_rows(v)+(uint64_t)offset*width,(size_t)length*width);
    nil_copy_hint("record_slice",(uint64_t)length*width,1,v);
    return copy;
}

/* Host queries are ordered effects, never borrowing/pure operations. */
void *nil_env(const NilSequence *name,uint64_t start,uint64_t end) {
    if(!name->length || memchr(name->data,'=',(size_t)name->length)) nil_fail(10,start,end);
    char *key=nil_path(name,start,end); nil_host_permission(start,end);
    const char *value=getenv(key); free(key);
    return nil_literal(value ? value : "",value ? (int64_t)strlen(value) : 0,start,end);
}
uint64_t nil_random(uint64_t start,uint64_t end) {
    nil_host_permission(start,end);
    if(!nil_random_ready) {
        const char *seed=getenv("NIL_RANDOM_SEED");
        if(seed) {
            uint64_t value=0;
            if(!*seed || (seed[0]=='0' && seed[1])) nil_fail(9,start,end);
            for(const unsigned char *p=(const unsigned char*)seed;*p;p++) {
                if(*p<'0' || *p>'9' || value>(UINT64_MAX-(*p-'0'))/10) nil_fail(9,start,end);
                value=value*10+(*p-'0');
            }
            nil_random_state=value;
        } else {
            FILE *f=fopen("/dev/urandom","rb"); if(!f) nil_fail(8,start,end);
            unsigned char bytes[8]; size_t n=fread(bytes,1,8,f); int closed=fclose(f);
            if(n!=8 || closed) nil_fail(8,start,end);
            nil_random_state=0;
            for(unsigned i=0;i<8;i++) nil_random_state|=(uint64_t)bytes[i]<<(8*i);
        }
        nil_random_ready=true;
    }
    uint64_t z=(nil_random_state+=UINT64_C(0x9e3779b97f4a7c15));
    z=(z^(z>>30))*UINT64_C(0xbf58476d1ce4e5b9);
    z=(z^(z>>27))*UINT64_C(0x94d049bb133111eb);
    return z^(z>>31);
}
static int nil_directory_compare(const void *a,const void *b) {
    return strcmp(*(char *const*)a,*(char *const*)b);
}
void *nil_directory(const NilSequence *path,uint64_t start,uint64_t end) {
    char *name=nil_path(path,start,end); nil_host_permission(start,end);
    DIR *dir=opendir(name); free(name); if(!dir) nil_fail(8,start,end);
    size_t count=0,capacity=16,used=0;
    char **names=malloc(capacity*sizeof(*names)); if(!names) nil_fail(6,start,end);
    for(;;) {
        errno=0; struct dirent *entry=readdir(dir);
        if(!entry) { if(errno) nil_fail(8,start,end); break; }
        if(!strcmp(entry->d_name,".") || !strcmp(entry->d_name,"..")) continue;
        if(count==capacity) {
            if(capacity>SIZE_MAX/2/sizeof(*names)) nil_fail(6,start,end);
            capacity*=2; char **grown=realloc(names,capacity*sizeof(*names));
            if(!grown) nil_fail(6,start,end); names=grown;
        }
        size_t len=strlen(entry->d_name);
        if(used>SIZE_MAX-len) nil_fail(6,start,end); used+=len;
        names[count]=malloc(len+1); if(!names[count]) nil_fail(6,start,end);
        memcpy(names[count++],entry->d_name,len+1);
    }
    if(closedir(dir)) nil_fail(8,start,end);
    qsort(names,count,sizeof(*names),nil_directory_compare);
    for(size_t i=1;i<count;i++) if(!strcmp(names[i-1],names[i])) nil_fail(13,start,end);
    uint64_t ec=4,bc=16;
    while(ec<count) { if(ec>NIL_MEMORY_LIMIT/128) nil_fail(6,start,end); ec*=2; }
    while(bc<used) { if(bc>NIL_MEMORY_LIMIT/2) nil_fail(6,start,end); bc*=2; }
    NilSequence *map=nil_allocate_capacity((int64_t)count,24+64*ec+bc,1,start,end);
    *nil_map_meta(map)=(NilMapMeta){ec,bc,(uint64_t)used};
    uint64_t offset=0;
    for(size_t i=0;i<count;i++) {
        size_t len=strlen(names[i]);
        // Hash directly over host scratch; no temporary arena allocation/root gap.
        uint64_t hash=UINT64_C(14695981039346656037);
        for(size_t j=0;j<len;j++) hash=(hash^(unsigned char)names[i][j])*UINT64_C(1099511628211);
        memcpy(nil_map_bytes(map)+offset,names[i],len);
        nil_map_entries(map)[i]=(NilMapEntry){hash,offset,(uint64_t)len,offset+len,0,0};
        offset+=len; free(names[i]);
    }
    free(names); nil_map_rehash(map); return map;
}
