/* Scalar numeric text: C locale, nearest-even, canonical NaN, no fast math.
   Private u128 interfaces use two uint64 words, avoiding C aggregate ABI variation. */
#include <math.h>
static double nil_canonical_double(double value) {
    uint64_t bits; memcpy(&bits,&value,8);
    if((bits&UINT64_C(0x7ff0000000000000))==UINT64_C(0x7ff0000000000000) && (bits&UINT64_C(0xfffffffffffff))) {
        bits=UINT64_C(0x7ff8000000000000);memcpy(&value,&bits,8);
    }
    return value;
}
static void nil_float_text(double value,char text[64]) {
    if(isnan(value)) {strcpy(text,"nan");return;}
    if(isinf(value)) {strcpy(text,signbit(value)?"-inf":"inf");return;}
    snprintf(text,64,"%.16e",value);
    char *exponent=strchr(text,'e');
    if(!exponent) abort();
    long number=strtol(exponent+1,NULL,10);
    snprintf(exponent+1,8,"%ld",number);
}
static bool nil_float_grammar(const unsigned char *s,uint64_t n) {
    uint64_t i=0;
    if(n==3 && (!memcmp(s,"nan",3) || !memcmp(s,"inf",3))) return true;
    if(n==4 && !memcmp(s,"-inf",4)) return true;
    if(i<n && (s[i]=='+' || s[i]=='-')) i++;
    uint64_t first=i;while(i<n && s[i]>='0' && s[i]<='9') i++;if(i==first)return false;
    if(i<n && s[i]=='.') {i++;first=i;while(i<n && s[i]>='0' && s[i]<='9') i++;if(i==first)return false;}
    if(i<n && (s[i]=='e' || s[i]=='E')) {i++;if(i<n && (s[i]=='+' || s[i]=='-'))i++;first=i;while(i<n && s[i]>='0' && s[i]<='9')i++;if(i==first)return false;}
    return i==n;
}
static double nil_float_parse(const unsigned char *s,uint64_t n,uint64_t start,uint64_t end) {
    if(!nil_float_grammar(s,n)) nil_fail(15,start,end);
    char *text=malloc((size_t)n+1);if(!text)nil_fail(6,start,end);
    memcpy(text,s,(size_t)n);text[n]=0;
    char *tail;double value=strtod(text,&tail);
    if(tail!=text+n) {free(text);nil_fail(15,start,end);}
    free(text);return nil_canonical_double(value);
}
double nil_parse_f64(const NilSequence *s,uint64_t start,uint64_t end) {return nil_float_parse(s->data,(uint64_t)s->length,start,end);}
void *nil_format_f64(double value,uint64_t start,uint64_t end) {char text[64];nil_float_text(value,text);return nil_literal(text,(int64_t)strlen(text),start,end);}
static bool nil_unsigned_parse(const unsigned char *s,uint64_t n,unsigned width,__uint128_t *out) {
    if(!n || (n>1 && s[0]=='0'))return false;
    __uint128_t value=0,maximum=width==64?UINT64_MAX:~(__uint128_t)0;
    for(uint64_t i=0;i<n;i++) {if(s[i]<'0' || s[i]>'9')return false;unsigned digit=s[i]-'0';if(value>(maximum-digit)/10)return false;value=value*10+digit;}
    *out=value;return true;
}
uint64_t nil_parse_u64(const NilSequence *s,uint64_t start,uint64_t end) {__uint128_t value;if(!nil_unsigned_parse(s->data,(uint64_t)s->length,64,&value))nil_fail(15,start,end);return (uint64_t)value;}
void nil_parse_u128(const NilSequence *s,uint64_t *out,uint64_t start,uint64_t end) {__uint128_t value;if(!nil_unsigned_parse(s->data,(uint64_t)s->length,128,&value))nil_fail(15,start,end);out[0]=(uint64_t)value;out[1]=(uint64_t)(value>>64);}
static void nil_wide_text(const uint64_t *parts,char text[40]) {
    __uint128_t value=((__uint128_t)parts[1]<<64)|parts[0];char reversed[40];unsigned n=0;
    do {reversed[n++]=(char)('0'+value%10);value/=10;}while(value);
    for(unsigned i=0;i<n;i++)text[i]=reversed[n-i-1];text[n]=0;
}
void *nil_format_u128(const uint64_t *parts,uint64_t start,uint64_t end) {char text[40];nil_wide_text(parts,text);return nil_literal(text,(int64_t)strlen(text),start,end);}
void *nil_format_u64(uint64_t value,uint64_t start,uint64_t end) {uint64_t parts[2]={value,0};return nil_format_u128(parts,start,end);}
static void nil_wide_argument(const char *text,uint64_t *out) {
    __uint128_t value;if(!nil_unsigned_parse((const unsigned char*)text,strlen(text),128,&value)) {fputs("E010 invalid u128 argument\n",stderr);exit(2);}out[0]=(uint64_t)value;out[1]=(uint64_t)(value>>64);
}
static uint64_t nil_unsigned_argument(const char *text,unsigned width) {
    __uint128_t value;if(!nil_unsigned_parse((const unsigned char*)text,strlen(text),width,&value)) {fputs("E010 invalid unsigned argument\n",stderr);exit(2);}return (uint64_t)value;
}
static uint64_t nil_float_argument(const char *text) {
    if(!nil_float_grammar((const unsigned char*)text,strlen(text))) {fputs("E010 invalid f64 argument\n",stderr);exit(2);}
    double value=nil_float_parse((const unsigned char*)text,strlen(text),UINT64_MAX,UINT64_MAX);uint64_t bits;memcpy(&bits,&value,8);return bits;
}
