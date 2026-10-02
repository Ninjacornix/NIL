#include <sys/stat.h>
#include <unistd.h>

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
#define NIL_SEQUENCE_OVERHEAD UINT64_C(40)
#define NIL_MEMORY_LIMIT UINT64_C(67108864)
_Static_assert(sizeof(NilSequence)==NIL_SEQUENCE_OVERHEAD, "sequence charge/layout mismatch");
typedef struct NilRoots {
    struct NilRoots *previous;
    NilSequence **slots;
    uint64_t count;
} NilRoots;
static _Thread_local NilRoots *nil_roots;
void *nil_roots_enter(NilSequence **slots,uint64_t count) {
    NilRoots *frame=malloc(sizeof(*frame));
    if(!frame) nil_fail(6,UINT64_MAX,UINT64_MAX);
    frame->previous=nil_roots; frame->slots=slots; frame->count=count;
    nil_roots=frame; return frame;
}
/* Slot updates are the only way to change execution roots. A zero-root payload
   survives transfer gaps until the next allocation/collection boundary. */
__attribute__((always_inline)) void nil_root_store(NilSequence **slot,NilSequence *value) {
    NilSequence *old=*slot;
    if(old==value) return;
    if(old && --old->roots==0) nil_live-=old->capacity*old->width+NIL_SEQUENCE_OVERHEAD;
    if(value && value->roots++==0) nil_live+=value->capacity*value->width+NIL_SEQUENCE_OVERHEAD;
    *slot=value;
}
void nil_roots_leave(NilRoots *frame) {
    if(nil_roots!=frame) abort();
    for(uint64_t i=0;i<frame->count;i++) nil_root_store(&frame->slots[i],NULL);
    nil_roots=frame->previous; free(frame);
}
static void nil_collect(void) {
    NilSequence **link=&nil_allocations;
    while(*link) {
        NilSequence *value=*link;
        if(!value->roots) {
            *link=value->next;
            nil_allocated-=value->capacity*value->width+NIL_SEQUENCE_OVERHEAD;
            free(value);
        } else link=&value->next;
    }
}
static NilSequence *nil_allocate_capacity(int64_t length, uint64_t capacity, uint64_t width, uint64_t start, uint64_t end) {
    if (length < 0 || capacity < (uint64_t)length || capacity > (NIL_MEMORY_LIMIT-NIL_SEQUENCE_OVERHEAD)/width) nil_fail(6,start,end);
    nil_collect();
    uint64_t bytes=capacity*width;
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
    nil_allocated=0; nil_live=0;
}
void *nil_make(int64_t length,int64_t fill,int64_t width,uint64_t start,uint64_t end) {
    if(length<0) nil_fail(6,start,end);
    if(width==1 && (fill<0 || fill>255)) nil_fail(7,start,end);
    NilSequence *value=nil_allocate(length,(uint64_t)width,start,end);
    if(width==1) memset(value->data,(unsigned char)fill,(size_t)length);
    else for(int64_t i=0;i<length;i++) ((int64_t*)value->data)[i]=fill;
    return value;
}
void *nil_literal(const void *bytes,int64_t length,uint64_t start,uint64_t end) {
    NilSequence *value=nil_allocate(length,1,start,end);
    if(length) memcpy(value->data,bytes,(size_t)length); return value;
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
int64_t nil_parse(const NilSequence *value,uint64_t start,uint64_t end) {
    if(value->length<1 || value->length>20) nil_fail(9,start,end);
    char text[32],canonical[32],*tail; memcpy(text,value->data,(size_t)value->length); text[value->length]=0;
    errno=0; intmax_t number=strtoimax(text,&tail,10);
    if(errno==ERANGE || tail!=text+value->length || number<INT64_MIN || number>INT64_MAX) nil_fail(9,start,end);
    int n=snprintf(canonical,sizeof(canonical),"%" PRId64,(int64_t)number);
    if(n!=value->length || memcmp(canonical,text,(size_t)n)) nil_fail(9,start,end);
    return (int64_t)number;
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
