#include <stddef.h>
#include <stdbool.h>
#include <sys/stat.h>
#include <unistd.h>
#include <dirent.h>
#include <errno.h>

/* Application runtime: execution-owned immutable sequences. No public pointer ABI. */
typedef struct NilSequence {
    struct NilSequence *next;
    int64_t length;
    uint64_t width;
    uint64_t roots;
    uint64_t capacity;
    unsigned char data[];
} NilSequence;
static _Thread_local NilSequence *nil_allocations;
static _Thread_local uint64_t nil_allocated;
static _Thread_local uint64_t nil_live;
static _Thread_local uint64_t nil_random_state;
static _Thread_local bool nil_random_ready;
#define NIL_SEQUENCE_OVERHEAD UINT64_C(40)
#define NIL_MEMORY_LIMIT UINT64_C(67108864)
_Static_assert(offsetof(NilSequence,data)==40 && _Alignof(NilSequence)==8, "sequence data ABI mismatch");
_Static_assert(sizeof(NilSequence)==NIL_SEQUENCE_OVERHEAD, "sequence charge/layout mismatch");
typedef struct NilRoots {
    struct NilRoots *previous;
    NilSequence **slots;
    uint64_t count;
} NilRoots;
static _Thread_local NilRoots *nil_roots;
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
void *nil_roots_enter(NilSequence **slots,uint64_t count,NilRoots *frame) {
    frame->previous=nil_roots; frame->slots=slots; frame->count=count;
    nil_roots=frame; return frame;
}
/* Slot updates are the only way to change execution roots. A zero-root payload
   survives transfer gaps until the next allocation/collection boundary. */
__attribute__((always_inline)) void nil_root_store(NilSequence **slot,NilSequence *value) {
    NilSequence *old=*slot;
    if(old==value) return;
    nil_drop(old);
    nil_retain(value);
    *slot=value;
}
void nil_roots_leave(NilRoots *frame) {
    if(nil_roots!=frame) abort();
    for(uint64_t i=0;i<frame->count;i++) nil_root_store(&frame->slots[i],NULL);
    nil_roots=frame->previous;
}
static void nil_collect(void) {
    NilSequence **link=&nil_allocations;
    while(*link) {
        NilSequence *value=*link;
        if(!value->roots) {
            *link=value->next;
            nil_allocated-=nil_owned_size(value);
            free(value);
        } else link=&value->next;
    }
}
/* Bound garbage as well as total physical capacity. Growing live graphs then
   trigger geometrically spaced sweeps; copy-heavy workloads do not accumulate
   large dead payloads that displace their working set from cache. */
static bool nil_should_collect(uint64_t reservation) {
#if !NIL_RECORD_BUFFERS
    // Closed-world typing proves there are no heap child edges. Keep the
    // established tiny flat-arena path, including immediate allocator reuse.
    (void)reservation; return true;
#else
    uint64_t garbage=nil_allocated-nil_live;
    uint64_t budget=nil_live/4;
    if(budget<UINT64_C(65536)) budget=UINT64_C(65536);
    return nil_allocated>NIL_MEMORY_LIMIT-reservation || garbage>=budget;
#endif
}
static NilSequence *nil_allocate_capacity(int64_t length, uint64_t capacity, uint64_t width, uint64_t start, uint64_t end) {
    if (length < 0 || capacity < (uint64_t)length || capacity > (NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width) nil_fail(6,start,end);
    uint64_t bytes=capacity*width;
    if (nil_should_collect(bytes+NIL_SEQUENCE_OVERHEAD)) nil_collect();
    if (nil_allocated > NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    NilSequence *value=malloc(sizeof(*value)+(size_t)bytes);
    if (!value) nil_fail(6,start,end);
    nil_allocated+=bytes+NIL_SEQUENCE_OVERHEAD; value->next=nil_allocations; value->length=length; value->width=width; value->roots=0; value->capacity=capacity;
    nil_allocations=value; return value;
}
static NilSequence *nil_allocate(int64_t length,uint64_t width,uint64_t start,uint64_t end) {
    return nil_allocate_capacity(length,length<0 ? 0 : (uint64_t)length,width,start,end);
}
static void nil_release(void) {
    while(nil_allocations) { NilSequence *next=nil_allocations->next; free(nil_allocations); nil_allocations=next; }
    nil_allocated=0; nil_live=0; nil_random_ready=false;
}
void *nil_make(int64_t length,int64_t fill,int64_t width,uint64_t start,uint64_t end) {
    if(length<0) nil_fail(6,start,end);
    if(width==1 && (fill<0 || fill>255)) nil_fail(7,start,end);
    NilSequence *value=nil_allocate(length,(uint64_t)width,start,end);
    if(width==1) memset(value->data,(unsigned char)fill,(size_t)length);
    else for(int64_t i=0;i<length;i++) ((int64_t*)value->data)[i]=fill;
    return value;
}
/* Adjacent singleton concat RHS: stack storage is never linked into the arena.
   Charge precisely the ordinary capacity, including the header, until the RHS
   root is dropped. No public value can retain this temporary. */
__attribute__((always_inline)) void *nil_make_temporary(NilSequence *value,int64_t fill,uint64_t width,uint64_t start,uint64_t end) {
    if(width==1 && (fill<0 || fill>255)) nil_fail(7,start,end);
    uint64_t charge=NIL_SEQUENCE_OVERHEAD+width;
    if(nil_allocated>NIL_MEMORY_LIMIT-charge) nil_collect();
    if(nil_allocated>NIL_MEMORY_LIMIT-charge) nil_fail(6,start,end);
    nil_allocated+=charge; nil_live+=charge;
    /* The sole adjacent consumer cannot escape or relocate this payload. A
       virtual root gives the ordinary live charge without a shadow-stack slot. */
    value->next=NULL; value->length=1; value->width=width; value->roots=1; value->capacity=1;
    if(width==1) value->data[0]=(unsigned char)fill;
    else ((int64_t*)value->data)[0]=fill;
    return value;
}
__attribute__((always_inline)) void nil_temporary_release(NilSequence *value) {
    if(value->roots!=1) abort();
    uint64_t charge=NIL_SEQUENCE_OVERHEAD+value->width;
    nil_allocated-=charge; nil_live-=charge; value->roots=0;
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
    if(value->width==1) copy->data[index]=(unsigned char)replacement;
    else ((int64_t*)copy->data)[index]=replacement;
    return copy;
}
/* Static chain proof admits this call; live-root uniqueness discharges aliases
   across callers, scopes and parallel state. Reserve the same semantic result
   charge as copying, even when no physical allocation is needed. */
__attribute__((always_inline)) void *nil_set_unique(NilSequence *value,int64_t index,int64_t replacement,uint64_t start,uint64_t end) {
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
    uint64_t bytes=value->capacity*value->width;
    if(nil_live>NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    if(value->roots==0) abort();
    if(value->roots!=1) return nil_set(value,index,replacement,start,end);
    if(value->width==1) value->data[index]=(unsigned char)replacement;
    else ((int64_t*)value->data)[index]=replacement;
    return value;
}
/* A scalar snapshot may move only after every original replacement check and
   before its write. The emitter proves there is no intervening observable work. */
__attribute__((always_inline)) void *nil_set_unique_capture(NilSequence *value,int64_t index,int64_t replacement,int64_t *original,uint64_t start,uint64_t end) {
    if(index<0 || index>=value->length) nil_fail(4,start,end);
    if(value->width==1 && (replacement<0 || replacement>255)) nil_fail(7,start,end);
    uint64_t bytes=value->capacity*value->width;
    if(nil_live>NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    *original=value->width==1 ? value->data[index] : ((int64_t*)value->data)[index];
    return nil_set_unique(value,index,replacement,start,end);
}
static uint64_t nil_concat_capacity(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/a->width;
    uint64_t needed=(uint64_t)a->length+(uint64_t)b->length;
    if(needed>maximum) nil_fail(6,start,end);
    if(needed<=a->capacity) return a->capacity;
    uint64_t occupied=3*NIL_SEQUENCE_OVERHEAD+b->capacity*b->width;
    uint64_t steady=occupied>NIL_MEMORY_LIMIT ? 0 : (NIL_MEMORY_LIMIT-occupied)/(2*a->width);
    uint64_t grown=a->capacity>steady/2 ? steady : a->capacity*2;
    return needed>grown ? needed : grown;
}
void *nil_concat(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t capacity=nil_concat_capacity(a,b,start,end);
    NilSequence *copy=nil_allocate_capacity(a->length+b->length,capacity,a->width,start,end);
    memcpy(copy->data,a->data,(size_t)a->length*a->width);
    memcpy(copy->data+(size_t)a->length*a->width,b->data,(size_t)b->length*b->width);
    return copy;
}
/* Last-use admission is static; active roots and a distinct RHS discharge all
   remaining aliases. Growing relocates only this one dead operand's root slot. */
__attribute__((always_inline)) void *nil_concat_unique(NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t capacity=nil_concat_capacity(a,b,start,end);
    uint64_t bytes=capacity*a->width;
    if(nil_live>NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    if(a->roots==0) abort();
    if(a->roots!=1 || a==b) return nil_concat(a,b,start,end);
    if(capacity!=a->capacity) {
        nil_collect();
        NilSequence **link=&nil_allocations;
        while(*link && *link!=a) link=&(*link)->next;
        if(!*link) abort();
        NilSequence **root=NULL;
        for(NilRoots *frame=nil_roots;frame;frame=frame->previous)
            for(uint64_t i=0;i<frame->count;i++) if(frame->slots[i]==a) root=&frame->slots[i];
        if(!root) abort();
        uint64_t delta=(capacity-a->capacity)*a->width;
        NilSequence *grown=realloc(a,sizeof(*a)+(size_t)bytes);
        if(!grown) nil_fail(6,start,end);
        grown->capacity=capacity; *link=grown; *root=grown;
        nil_live+=delta; nil_allocated+=delta; a=grown;
    }
    memcpy(a->data+(size_t)a->length*a->width,b->data,(size_t)b->length*b->width);
    a->length+=b->length;
    return a;
}
void *nil_slice(const NilSequence *value,int64_t offset,int64_t length,uint64_t start,uint64_t end) {
    if(offset<0 || length<0 || offset>value->length || length>value->length-offset) nil_fail(4,start,end);
    NilSequence *copy=nil_allocate(length,value->width,start,end);
    if(length) memcpy(copy->data,value->data+(size_t)offset*value->width,(size_t)length*value->width);
    return copy;
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
void *nil_parsebuf(const NilSequence *text,const NilSequence *delimiters,uint64_t start,uint64_t end) {
    bool separators[256]={false};
    for(int64_t i=0;i<delimiters->length;i++) separators[delimiters->data[i]]=true;
    int64_t count=0;
    for(int64_t i=0;i<text->length;i++) if(separators[text->data[i]]) count++;
    if(text->length && !separators[text->data[text->length-1]]) count++;
    /* Reserve exact result capacity before conversion: quota failure precedes
       canonical-field failure. Text and delimiter operands remain rooted. */
    NilSequence *result=nil_allocate(count,8,start,end);
    int64_t offset=0,index=0;
    for(int64_t i=0;i<text->length;i++) if(separators[text->data[i]]) {
        ((int64_t*)result->data)[index++]=nil_decimal(text->data+offset,i-offset,start,end);
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
       byte beyond the budget to preserve E013-before-E015 for an oversized
       successful prefix, including a stream that subsequently fails. */
    size_t available=nil_allocated>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD ? 0 :
        (size_t)(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD-nil_allocated);
    size_t maximum=available+1,capacity=maximum<65536 ? maximum : 65536,length=0;
    struct stat info;
    if(fstat(fileno(file),&info)==0 && S_ISREG(info.st_mode) && info.st_size>=0) {
        uint64_t hint=(uint64_t)info.st_size;
        capacity=hint>=maximum ? maximum : (size_t)hint+1;
    }
    /* Unpublished host scratch becomes the arena payload only after successful
       I/O; admission retains the original I/O-before-final-allocation ordering. */
    NilSequence *value=malloc(sizeof(*value)+capacity); if(!value) nil_fail(6,start,end);
    for(;;) {
        if(length==capacity) {
            size_t next_capacity=capacity>maximum/2 ? maximum : capacity*2;
            NilSequence *next=realloc(value,sizeof(*value)+next_capacity);
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
    if(nil_allocated>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    NilSequence *exact=realloc(value,sizeof(*value)+length);
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
static uint64_t nil_map_hash(const NilSequence *key) {
    uint64_t h=UINT64_C(14695981039346656037);
    for(int64_t i=0;i<key->length;i++) h=(h^key->data[i])*UINT64_C(1099511628211);
    return h;
}
/* Insertion order lives in entries, never in hash-table traversal order. */
static uint64_t nil_map_find(const NilSequence *m,const NilSequence *key,uint64_t hash,uint64_t *bucket) {
    uint64_t mask=2*nil_map_meta(m)->entries-1, slot=hash&mask;
    uint64_t *table=nil_map_buckets(m); NilMapEntry *entries=nil_map_entries(m);
    while(table[slot]) {
        uint64_t i=table[slot]-1; NilMapEntry *e=&entries[i];
        if(e->hash==hash && e->key_len==(uint64_t)key->length && !memcmp(nil_map_bytes(m)+e->key,key->data,e->key_len)) { *bucket=slot;return i; }
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
    (void)start;(void)end;uint64_t slot;return nil_map_find(m,key,nil_map_hash(key),&slot)!=UINT64_MAX;
}
int64_t nil_map_size(const NilSequence *m,uint64_t start,uint64_t end) { (void)start;(void)end;return m->length; }
static const NilMapEntry *nil_map_get_entry(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) {
    uint64_t slot,i=nil_map_find(m,key,nil_map_hash(key),&slot);
    if(i==UINT64_MAX) nil_fail(12,start,end);
    return &nil_map_entries(m)[i];
}
int64_t nil_map_get_int(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) { return nil_map_get_entry(m,key,start,end)->number; }
void *nil_map_get_bytes(const NilSequence *m,const NilSequence *key,uint64_t start,uint64_t end) {
    const NilMapEntry *e=nil_map_get_entry(m,key,start,end);
    return nil_literal(nil_map_bytes(m)+e->value,(int64_t)e->value_len,start,end);
}
void *nil_map_key(const NilSequence *m,int64_t index,uint64_t start,uint64_t end) {
    if(index<0 || index>=m->length) nil_fail(4,start,end);
    const NilMapEntry *e=&nil_map_entries(m)[index];
    return nil_literal(nil_map_bytes(m)+e->key,(int64_t)e->key_len,start,end);
}
/* Last-use + unique live root allows writes; all other cases copy. Even an
   in-place update reserves the same old+result semantic charge as copying. */
static NilSequence *nil_map_update(NilSequence *m,const NilSequence *key,int64_t number,const unsigned char *value,uint64_t value_len,bool insert,bool unique,uint64_t start,uint64_t end) {
    uint64_t slot, hash=nil_map_hash(key), i=nil_map_find(m,key,hash,&slot);
    if(insert && i!=UINT64_MAX) nil_fail(13,start,end);
    uint64_t old_len=i==UINT64_MAX?0:nil_map_entries(m)[i].value_len;
    uint64_t used=nil_map_meta(m)->used-old_len+value_len+(i==UINT64_MAX?(uint64_t)key->length:0);
    uint64_t count=(uint64_t)m->length+(i==UINT64_MAX);
    uint64_t ec=nil_map_meta(m)->entries, bc=nil_map_meta(m)->bytes;
    while(ec<count) ec*=2;
    while(bc<used) bc*=2;
    uint64_t capacity=24+64*ec+bc;
    if(capacity>NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD || nil_live>NIL_MEMORY_LIMIT-capacity-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
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
        nil_map_rehash(out);
        if(i!=UINT64_MAX) return out;
        (void)nil_map_find(out,key,hash,&slot);
    }
    if(i==UINT64_MAX) {
        i=(uint64_t)out->length++;
        uint64_t offset=nil_map_meta(out)->used;
        if(key->length) memcpy(nil_map_bytes(out)+offset,key->data,(size_t)key->length);
        nil_map_entries(out)[i]=(NilMapEntry){hash,offset,(uint64_t)key->length,offset+(uint64_t)key->length,value_len,number};
        nil_map_buckets(out)[slot]=i+1;nil_map_meta(out)->used=used;
    }
    NilMapEntry *e=&nil_map_entries(out)[i];e->number=number;
    if(value_len) memcpy(nil_map_bytes(out)+e->value,value,value_len);
    return out;
}
#define NIL_MAP_UPDATE(name,insert,unique) \
void *nil_map_##name##_int(NilSequence *m,const NilSequence *key,int64_t value,uint64_t start,uint64_t end) { return nil_map_update(m,key,value,NULL,0,insert,unique,start,end); } \
void *nil_map_##name##_bytes(NilSequence *m,const NilSequence *key,const NilSequence *value,uint64_t start,uint64_t end) { return nil_map_update(m,key,0,value->data,(uint64_t)value->length,insert,unique,start,end); }
NIL_MAP_UPDATE(insert,true,false)
NIL_MAP_UPDATE(insert_unique,true,true)
NIL_MAP_UPDATE(put,false,false)
NIL_MAP_UPDATE(put_unique,false,true)
/* Scalar-record map slots have a specified little-endian encoding, independent
   of target byte order. No pointers/child aliases enter the packed payload. */
void nil_map_get_record(const NilSequence *m,const NilSequence *key,uint64_t *out,uint64_t slots,uint64_t start,uint64_t end) {
    const NilMapEntry *entry=nil_map_get_entry(m,key,start,end);
    if(slots>4096 || entry->value_len!=slots*8) abort();
    const unsigned char *data=nil_map_bytes(m)+entry->value;
    for(uint64_t i=0;i<slots;i++) { uint64_t word=0;for(unsigned b=0;b<8;b++) word|=(uint64_t)data[i*8+b]<<(8*b);out[i]=word; }
}
void *nil_map_update_record(NilSequence *m,const NilSequence *key,const uint64_t *value,uint64_t slots,bool insert,bool unique,uint64_t start,uint64_t end) {
    if(slots>4096) abort();
    unsigned char bytes[4096*8];
    for(uint64_t i=0;i<slots;i++) for(unsigned b=0;b<8;b++) bytes[i*8+b]=(unsigned char)(value[i]>>(8*b));
    return nil_map_update(m,key,0,bytes,slots*8,insert,unique,start,end);
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
    uint64_t bytes=value->capacity*value->width;
    if(nil_live>NIL_MEMORY_LIMIT-bytes-NIL_SEQUENCE_OVERHEAD) nil_fail(6,start,end);
    if(!unique || value->roots!=1) {
        NilSequence *copy=nil_allocate_capacity(value->length,value->capacity,value->width,start,end);
        memcpy(copy->data,value->data,(size_t)bytes); value=copy;
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
static NilSequence *nil_record_allocate(int64_t length,uint64_t capacity,const uint64_t *desc,uint64_t start,uint64_t end) {
    uint64_t width=desc[0]*8;
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width-1;
    if(length<0 || capacity<(uint64_t)length || capacity>maximum) nil_fail(6,start,end);
    uint64_t bytes=(capacity+1)*width+NIL_SEQUENCE_OVERHEAD;
    if(nil_should_collect(bytes)) nil_collect();
    if(nil_allocated>NIL_MEMORY_LIMIT-bytes) nil_fail(6,start,end);
    NilSequence *v=malloc((size_t)bytes);if(!v) nil_fail(6,start,end);
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
}
void *nil_record_set(NilSequence *v,int64_t index,const void *replacement,bool unique,uint64_t start,uint64_t end) {
    if(index<0 || index>=v->length) nil_fail(4,start,end);
    uint64_t bytes=nil_owned_size(v),width=v->width & ~NIL_RECORD_TAG;
    if(nil_live>NIL_MEMORY_LIMIT-bytes) nil_fail(6,start,end);
    const uint64_t *desc=nil_record_descriptor(v);
    if(!unique || v->roots!=1) {
        NilSequence *copy=nil_record_allocate(v->length,v->capacity,desc,start,end);
        memcpy(nil_record_rows(copy),nil_record_rows(v),(size_t)v->length*width);
        memcpy(nil_record_rows(copy)+(uint64_t)index*width,replacement,(size_t)width);
        return copy;
    }
    void *row=nil_record_rows(v)+(uint64_t)index*width;
    /* Retain replacements before releasing old aliases; no collection inside. */
    nil_row_roots(desc,replacement,true);nil_row_roots(desc,row,false);
    memcpy(row,replacement,(size_t)width);return v;
}
static uint64_t nil_record_capacity(const NilSequence *a,const NilSequence *b,uint64_t start,uint64_t end) {
    uint64_t width=a->width & ~NIL_RECORD_TAG;
    uint64_t maximum=(NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width-1;
    uint64_t needed=(uint64_t)a->length+(uint64_t)b->length;
    if(needed>maximum) nil_fail(6,start,end);
    if(needed<=a->capacity)return a->capacity;
    uint64_t occupied=3*(NIL_SEQUENCE_OVERHEAD+width)+b->capacity*width;
    uint64_t steady=occupied>NIL_MEMORY_LIMIT ? 0 : (NIL_MEMORY_LIMIT-occupied)/(2*width);
    uint64_t grown=a->capacity>steady/2 ? steady : a->capacity*2;
    return needed>grown ? needed : grown;
}
void *nil_record_concat(NilSequence *a,const NilSequence *b,bool unique,uint64_t start,uint64_t end) {
    uint64_t cap=nil_record_capacity(a,b,start,end),width=a->width & ~NIL_RECORD_TAG;
    uint64_t bytes=NIL_SEQUENCE_OVERHEAD+(cap+1)*width;
    if(nil_live>NIL_MEMORY_LIMIT-bytes) nil_fail(6,start,end);
    if(!unique || a->roots!=1 || a==b) {
        NilSequence *copy=nil_record_allocate(a->length+b->length,cap,nil_record_descriptor(a),start,end);
        memcpy(nil_record_rows(copy),nil_record_rows(a),(size_t)a->length*width);
        memcpy(nil_record_rows(copy)+(uint64_t)a->length*width,nil_record_rows(b),(size_t)b->length*width);
        return copy;
    }
    if(cap!=a->capacity) {
        nil_collect();NilSequence **link=&nil_allocations;
        while(*link && *link!=a)link=&(*link)->next;if(!*link)abort();
        NilSequence **root=NULL;
        for(NilRoots *frame=nil_roots;frame;frame=frame->previous)
            for(uint64_t i=0;i<frame->count;i++)if(frame->slots[i]==a)root=&frame->slots[i];
        if(!root)abort();uint64_t delta=(cap-a->capacity)*width;
        NilSequence *grown=realloc(a,(size_t)bytes);if(!grown)nil_fail(6,start,end);
        grown->capacity=cap;*link=grown;*root=grown;nil_live+=delta;nil_allocated+=delta;a=grown;
    }
    const uint64_t *desc=nil_record_descriptor(a);
    for(int64_t i=0;i<b->length;i++)nil_row_roots(desc,nil_record_rows(b)+(uint64_t)i*width,true);
    memcpy(nil_record_rows(a)+(uint64_t)a->length*width,nil_record_rows(b),(size_t)b->length*width);
    a->length+=b->length;return a;
}
void *nil_record_slice(const NilSequence *v,int64_t offset,int64_t length,uint64_t start,uint64_t end) {
    if(offset<0 || length<0 || offset>v->length || length>v->length-offset)nil_fail(4,start,end);
    NilSequence *copy=nil_record_allocate(length,length<0 ? 0 : (uint64_t)length,nil_record_descriptor(v),start,end);
    uint64_t width=v->width & ~NIL_RECORD_TAG;
    memcpy(nil_record_rows(copy),nil_record_rows(v)+(uint64_t)offset*width,(size_t)length*width);
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
