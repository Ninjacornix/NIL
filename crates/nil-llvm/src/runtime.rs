use crate::Options;

pub fn source(options: &Options, arity: usize) -> String {
    let parameters = (0..arity).map(|_| ", int64_t").collect::<String>();
    let arguments = (0..arity)
        .map(|i| format!(", values[{i}]"))
        .collect::<String>();
    RUNTIME
        .replace("$ENTRY", &options.entry.0.to_string())
        .replace("$PARAMETERS", &parameters)
        .replace("$ARGUMENTS", &arguments)
        .replace("$ARITY", &arity.to_string())
        .replace("$STEPS", &options.steps.to_string())
        .replace("$DEPTH", &options.call_depth.to_string())
}
const RUNTIME: &str = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <time.h>

typedef struct { uint64_t fuel, depth, limit; } NilContext;
_Static_assert(sizeof(NilContext) == 24 && _Alignof(NilContext) == 8, "unsupported context ABI");
extern int64_t nil_fn$ENTRY(NilContext *, uint64_t, uint64_t$PARAMETERS);
_Noreturn void nil_fail(uint32_t reason, uint64_t start, uint64_t end) {
    const char *messages[] = {"instruction budget exhausted", "call depth limit exceeded", "signed integer overflow", "division by zero", "array index out of bounds", "bool entry argument must be 0 or 1", "dynamic allocation limit exceeded", "byte value must be 0..255", "file I/O failed", "invalid decimal i64", "path contains NUL", "host I/O is not enabled", "missing map key", "duplicate map key", "numeric conversion out of range", "invalid numeric text"};
    unsigned code = reason >= 6 ? reason + 7 : reason == 4 ? 12 : reason == 5 ? 7 : reason < 2 ? 8 : 9;
    if (start != UINT64_MAX) fprintf(stderr, "E%03u @%" PRIu64 "..%" PRIu64 " %s\n", code, start, end, messages[reason]);
    else fprintf(stderr, "E%03u %s\n", code, messages[reason]);
    exit(1);
}
static int64_t argument(const char *text) {
    char *end;
    errno=0;
    intmax_t value=strtoimax(text,&end,10);
    if (text==end || *end!='\0' || errno==ERANGE || value<INT64_MIN || value>INT64_MAX
        || text[0]==' ' || text[0]=='\t' || text[0]=='\n' || text[0]=='\r' || text[0]=='\v' || text[0]=='\f') {
        fprintf(stderr,"E010 invalid i64 argument\n");exit(2);
    }
    return (int64_t)value;
}
static uint64_t now(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC,&t)!=0) { perror("clock_gettime");exit(1); }
    return (uint64_t)t.tv_sec*UINT64_C(1000000000)+(uint64_t)t.tv_nsec;
}
static int64_t invoke(const int64_t *values) {
    NilContext ctx={UINT64_C($STEPS),0,UINT64_C($DEPTH)};
    return nil_fn$ENTRY(&ctx,UINT64_MAX,UINT64_MAX$ARGUMENTS);
}
int main(int argc,char **argv) {
    int offset=1;
    int benchmark=argc>1 && strcmp(argv[1],"--bench")==0;
    uint64_t iterations=1;
    if (benchmark) {
        if (argc<3) { fprintf(stderr,"E010 --bench requires an iteration count\n");return 2; }
        int64_t count=argument(argv[2]);
        if (count<1 || count>10000000) { fprintf(stderr,"E010 iterations must be 1..10000000\n");return 2; }
        iterations=(uint64_t)count;offset=3;
    }
    if (argc-offset!=$ARITY) {
        fprintf(stderr,"E006 entry arity mismatch expected:%u got:%d\n",(unsigned)$ARITY,argc-offset);return 1;
    }
    int64_t values[$ARITY+1];
    for (unsigned i=0;i<$ARITY;i++) values[i]=argument(argv[offset+(int)i]);
    if (!benchmark) { printf("%" PRId64 "\n",invoke(values));return 0; }
    int64_t expected=invoke(values);
    for (unsigned i=0;i<1000;i++) if (invoke(values)!=expected) return 1;
    uint64_t start=now();
    for (uint64_t i=0;i<iterations;i++) if (invoke(values)!=expected) return 1;
    double ns=(double)(now()-start)/(double)iterations;
    printf("{\"result\":%" PRId64 ",\"ns_per_call\":%.6f}\n",expected,ns);
    return 0;
}
"#;

pub fn typed_source(options: &Options, function: &nil_hir::Function) -> String {
    let arity: usize = function.parameters.iter().map(|ty| ty.slots()).sum();
    let result_len = function.result_type.slots();
    let prefix = RUNTIME
        .split("static int64_t invoke")
        .next()
        .unwrap()
        .replace(
            "extern int64_t nil_fn$ENTRY(NilContext *, uint64_t, uint64_t$PARAMETERS);",
            "extern void nil_entry(NilContext *, const int64_t *, int64_t *);",
        );
    let printer = match function.result_type {
        nil_hir::Type::Record(..)
        | nil_hir::Type::MapRecord(..)
        | nil_hir::Type::RecordBuffer(..) => {
            unreachable!("record entry requires wrapper")
        }
        nil_hir::Type::U64 | nil_hir::Type::U128 | nil_hir::Type::F64 => {
            unreachable!("application runtime handles numeric values")
        }
        nil_hir::Type::I64 => "printf(\"%\" PRId64, result[0]);".to_string(),
        nil_hir::Type::Bool => "fputs(result[0] ? \"true\" : \"false\", stdout);".to_string(),
        nil_hir::Type::Buffer
        | nil_hir::Type::Bytes
        | nil_hir::Type::MapI64
        | nil_hir::Type::MapBytes => {
            unreachable!("application runtime handles dynamic values")
        }
        nil_hir::Type::Array(len) => format!(
            "putchar('['); for (unsigned i=0; i<{len}; i++) {{ if(i) putchar(','); printf(\"%\" PRId64,result[i]); }} putchar(']');"
        ),
    };
    let main = r#"
static void invoke(const int64_t *values, int64_t *result) {
    NilContext ctx={UINT64_C($STEPS),0,UINT64_C($DEPTH)};
    nil_entry(&ctx,values,result);
}
static void print_result(const int64_t *result) { $PRINTER }
int main(int argc, char **argv) {
    int benchmark=argc>1 && strcmp(argv[1],"--bench")==0;
    int offset=benchmark?3:1;
    uint64_t iterations=1;
    if (benchmark) {
        if(argc<3) { fputs("E010 --bench requires an iteration count\n",stderr); return 2; }
        int64_t count=argument(argv[2]);
        if(count<1 || count>10000000) { fputs("E010 iterations must be 1..10000000\n",stderr); return 2; }
        iterations=(uint64_t)count;
    }
    if(argc-offset!=$ARITY) {
        fprintf(stderr,"E006 entry arity mismatch expected:%u got:%d\n",(unsigned)$ARITY,argc-offset);return 1;
    }
    int64_t values[$ARITY+1];
    for(unsigned i=0;i<$ARITY;i++) values[i]=argument(argv[offset+(int)i]);
    int64_t expected[$RESULT_STORAGE]={0}, result[$RESULT_STORAGE]={0};
    invoke(values,expected);
    if(!benchmark) { print_result(expected); putchar('\n'); return 0; }
    for(unsigned i=0;i<1000;i++) { invoke(values,result); if(memcmp(result,expected,$RESULT_BYTES)) return 1; }
    uint64_t start=now();
    for(uint64_t i=0;i<iterations;i++) { invoke(values,result); if(memcmp(result,expected,$RESULT_BYTES)) return 1; }
    double ns=(double)(now()-start)/(double)iterations;
    fputs("{\"result\":",stdout); print_result(expected);
    printf(",\"ns_per_call\":%.6f}\n",ns); return 0;
}
"#;
    format!("{prefix}{main}")
        .replace("$STEPS", &options.steps.to_string())
        .replace("$DEPTH", &options.call_depth.to_string())
        .replace("$ARITY", &arity.to_string())
        .replace("$RESULT_STORAGE", &result_len.max(1).to_string())
        .replace("$RESULT_BYTES", &(result_len * 8).to_string())
        .replace("$PRINTER", &printer)
}

pub fn application_source(
    options: &Options,
    function: &nil_hir::Function,
    record_buffers: bool,
) -> String {
    use nil_hir::Type;
    let slots: usize = function.parameters.iter().map(|t| t.slots()).sum();
    let arity: usize = function
        .parameters
        .iter()
        .map(|t| if *t == Type::U128 { 1 } else { t.slots() })
        .sum();
    let mut argument = 0;
    let mut slot = 0;
    let mut inputs = String::new();
    for parameter in &function.parameters {
        match parameter {
            Type::MapI64 | Type::MapBytes => inputs.push_str(&format!("if(strcmp(argv[1+{argument}],\"{{}}\")) {{ fputs(\"E010 map entry arguments must be {{}}\\n\",stderr);exit(2); }} values[{slot}]=(int64_t)(intptr_t)nil_map(UINT64_MAX,UINT64_MAX);\n")),
            Type::U64 => inputs.push_str(&format!("values[{slot}]=(int64_t)nil_unsigned_argument(argv[1+{argument}],64);\n")),
            Type::U128 => inputs.push_str(&format!("nil_wide_argument(argv[1+{argument}],(uint64_t*)&values[{slot}]);\n")),
            Type::F64 => inputs.push_str(&format!("values[{slot}]=(int64_t)nil_float_argument(argv[1+{argument}]);\n")),
            Type::Buffer => inputs.push_str(&format!("values[{slot}]=(int64_t)(intptr_t)nil_buffer_argument(argv[1+{argument}]);\n")),
            Type::Bytes => inputs.push_str(&format!("values[{slot}]=(int64_t)(intptr_t)nil_literal(argv[1+{argument}],(int64_t)strlen(argv[1+{argument}]),UINT64_MAX,UINT64_MAX);\n")),
            _ => for i in slot..slot+parameter.slots() { inputs.push_str(&format!("values[{i}]=argument(argv[1+{argument}+{i}-{slot}]);\n")); },
        }
        if parameter.is_dynamic() {
            inputs.push_str(&format!(
                "nil_root_store(&input_roots[{slot}],(NilSequence*)(intptr_t)values[{slot}]);\n"
            ));
        }
        argument += if *parameter == Type::U128 {
            1
        } else {
            parameter.slots()
        };
        slot += parameter.slots();
    }
    let printer = match function.result_type {
        Type::Record(..) | Type::MapRecord(..) | Type::RecordBuffer(..) => unreachable!("record entry requires wrapper"),
        Type::MapI64 => "nil_map_print((NilSequence*)(intptr_t)result[0],false);".into(),
        Type::MapBytes => "nil_map_print((NilSequence*)(intptr_t)result[0],true);".into(),
        Type::Bytes => "NilSequence *value=(NilSequence*)(intptr_t)result[0]; if(fwrite(value->data,1,(size_t)value->length,stdout)!=(size_t)value->length) nil_fail(8,UINT64_MAX,UINT64_MAX);".to_string(),
        Type::Buffer => "NilSequence *value=(NilSequence*)(intptr_t)result[0]; putchar('['); for(int64_t i=0;i<value->length;i++) { if(i) putchar(','); printf(\"%\" PRId64,((int64_t*)value->data)[i]); } putchar(']');".to_string(),
        Type::U64 => "printf(\"%\" PRIu64,(uint64_t)result[0]);".into(),
        Type::U128 => "char text[40]; nil_wide_text((uint64_t*)result,text);fputs(text,stdout);".into(),
        Type::F64 => "double value;memcpy(&value,result,8);char text[64];nil_float_text(value,text);fputs(text,stdout);".into(),
        Type::I64 => "printf(\"%\" PRId64,result[0]);".into(),
        Type::Bool => "fputs(result[0]?\"true\":\"false\",stdout);".into(),
        Type::Array(n) => format!("putchar('['); for(unsigned i=0;i<{n};i++) {{ if(i) putchar(','); printf(\"%\" PRId64,result[i]); }} putchar(']');"),
    };
    let prefix = RUNTIME
        .split("static int64_t invoke")
        .next()
        .unwrap()
        .replace(
            "extern int64_t nil_fn$ENTRY(NilContext *, uint64_t, uint64_t$PARAMETERS);",
            "extern void nil_entry(NilContext *, const int64_t *, int64_t *);",
        );
    format!(
        "{prefix}\n#define NIL_RECORD_BUFFERS {}\n{}\nint main(int argc,char **argv) {{\nif(argc-1!={arity}) {{ fprintf(stderr,\"E006 entry arity mismatch expected:{arity} got:%d\\n\",argc-1);return 1; }}\nint64_t values[{}]={{0}}, result[{}]={{0}};\nNilSequence *input_roots[2*{slots}+1]={{0}}; NilRoots input_storage; NilRoots *input_frame=nil_roots_enter(input_roots,{slots},&input_storage);\n{inputs}\nnil_roots_leave(input_frame);\nNilContext ctx={{UINT64_C({}),0,UINT64_C({})}};\nnil_entry(&ctx,values,result);\n{printer}\nputchar('\\n'); nil_release(); return 0;\n}}\n",
        u8::from(record_buffers),
        format_args!(
            "{}\n{}",
            include_str!("application.c"),
            include_str!("numeric.c")
        ),
        slots.max(1),
        function.result_type.slots().max(1),
        options.steps,
        options.call_depth
    )
}
