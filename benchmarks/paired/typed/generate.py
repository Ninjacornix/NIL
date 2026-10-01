"""Reproduce the fixed-array corpus and independent expected answers."""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
GET = '''template<std::size_t N>
std::int64_t get(const std::array<std::int64_t,N>& a, std::int64_t i) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    return a[static_cast<std::size_t>(i)];
}
'''
PUT = '''template<std::size_t N>
std::array<std::int64_t,N> put(std::array<std::int64_t,N> a, std::int64_t i, std::int64_t v) {
    if (i < 0 || static_cast<std::uint64_t>(i) >= N) std::abort();
    a[static_cast<std::size_t>(i)] = v;
    return a;
}
'''
PY_PUT = '''def put(a, i, value):
    result = a.copy()
    result[i] = value
    return result

'''


def oracle(name, args):
    a = args[0]
    if name == 'sum': return sum(a)
    if name == 'dot': return sum(x*y for x,y in zip(a,args[1]))
    if name == 'max': return max(a)
    if name == 'search': return a.index(args[1]) if args[1] in a else -1
    if name == 'reverse': return list(reversed(a))
    if name == 'prefix':
        import itertools
        return list(itertools.accumulate(a))
    if name == 'predicate': return a > 0
    raise ValueError(name)

def local_sources(name, n, py):
    """Fresh local mutation preserves the observable value contract."""
    arr=f'std::array<std::int64_t,{n}>'
    body=(f'{arr} result{{}}; for(std::int64_t i=0;i<{n};i++) result[i]=get(a,{n}-i-1); return result;'
          if name == 'reverse' else f'{arr} result{{}}; std::int64_t total=0; for(std::int64_t i=0;i<{n};i++) {{ result[i]=total+get(a,i); total+=get(a,i); }} return result;')
    cpp='#include <array>\n#include <cstdint>\n#include <cstdlib>\n'+GET+f'{arr} program({arr} a) {{\n    {body}\n}}\n'
    py='def program'+py.split('def program',1)[1].replace('result = put(result, i, a[len(a)-i-1])','result[i] = a[len(a)-i-1]').replace('result = put(result, i, total+a[i])','result[i] = total+a[i]')
    return cpp,py


def corpus():
    cases = []
    for n in [8,32,128,256]:
        arr = f'std::array<std::int64_t,{n}>'
        for name in ['sum','dot','max','search','reverse','prefix']:
            ids = f'{name}-{n}'
            params = [n,n] if name == 'dot' else [n,'i'] if name == 'search' else [n]
            result = n if name in ['reverse','prefix'] else 'i'
            nil = {
                'sum':f'({n})=@(a,0,0;b<#a;a,b+1,c+a[b];c)\n',
                'dot':f'({n},{n})=@(a,b,0,0;c<#a;a,b,c+1,d+a[c]*b[c];d)\n',
                'max':f'({n})=@(a,1,a[0];b<#a;a,b+1,a[b]>c?a[b]:c;c)\n',
                'search':f'({n},i)=@(a,b,0,#a;c<d;a,b,a[(c+d)/2]<b?(c+d)/2+1:c,a[(c+d)/2]<b?d:(c+d)/2;c<#a?a[c]==b?c:-1:-1)\n',
                'reverse':f'({n}):{n}=@(a,[0;{n}],0;c<#a;a,b[c:a[#a-c-1]],c+1;b)\n',
                'prefix':f'({n}):{n}=@(a,[0;{n}],0,0;c<#a;a,b[c:d+a[c]],c+1,d+a[c];b)\n',
            }[name]
            py = {
                'sum':'def program(a):\n    total = 0\n    for i in range(len(a)):\n        total += a[i]\n    return total\n',
                'dot':'def program(a, b):\n    total = 0\n    for i in range(len(a)):\n        total += a[i]*b[i]\n    return total\n',
                'max':'def program(a):\n    best = a[0]\n    for i in range(1, len(a)):\n        best = a[i] if a[i] > best else best\n    return best\n',
                'search':'def program(a, target):\n    lo, hi = 0, len(a)\n    while lo < hi:\n        mid = (lo+hi)//2\n        if a[mid] < target:\n            lo = mid+1\n        else:\n            hi = mid\n    return lo if lo < len(a) and a[lo] == target else -1\n',
                'reverse':PY_PUT+'def program(a):\n    result = [0]*len(a)\n    for i in range(len(a)):\n        result = put(result, i, a[len(a)-i-1])\n    return result\n',
                'prefix':PY_PUT+'def program(a):\n    result = [0]*len(a)\n    total = 0\n    for i in range(len(a)):\n        result = put(result, i, total+a[i])\n        total += a[i]\n    return result\n',
            }[name]
            cpp_args = f'{arr} a, {arr} b' if name == 'dot' else f'{arr} a, std::int64_t target' if name == 'search' else f'{arr} a'
            cpp_body = {
                'sum':f'std::int64_t total=0; for(std::int64_t i=0;i<{n};i++) total+=get(a,i); return total;',
                'dot':f'std::int64_t total=0; for(std::int64_t i=0;i<{n};i++) total+=get(a,i)*get(b,i); return total;',
                'max':f'std::int64_t best=get(a,0); for(std::int64_t i=1;i<{n};i++) best=get(a,i)>best?get(a,i):best; return best;',
                'search':f'std::int64_t lo=0,hi={n}; while(lo<hi) {{ auto mid=(lo+hi)/2; if(get(a,mid)<target) lo=mid+1; else hi=mid; }} return lo<{n} && get(a,lo)==target?lo:-1;',
                'reverse':f'{arr} result{{}}; for(std::int64_t i=0;i<{n};i++) result=put(result,i,get(a,{n}-i-1)); return result;',
                'prefix':f'{arr} result{{}}; std::int64_t total=0; for(std::int64_t i=0;i<{n};i++) {{ result=put(result,i,total+get(a,i)); total+=get(a,i); }} return result;',
            }[name]
            cpp_function = f'{arr if isinstance(result,int) else "std::int64_t"} program({cpp_args}) {{\n    {cpp_body}\n}}\n'
            cpp = '#include <array>\n#include <cstdint>\n#include <cstdlib>\n'+GET+(PUT if isinstance(result,int) else '')+cpp_function
            values = [i*3-n for i in range(n)]
            inputs = [values, list(reversed(values))] if name == 'dot' else [values,values[n//2]] if name == 'search' else [values]
            checks = [inputs]
            if name == 'search': checks.extend([[values,values[0]-1],[values,values[-1]+1],[values,values[n//2]+1]])
            elif name == 'dot': checks.extend([[[0]*n,[1]*n],[[1]*n,[-1]*n]])
            else: checks.extend([[[0]*n],[[-5]*n]])
            for ext,text in [('nil',nil),('py',py),('cpp',cpp),('body.cpp',cpp_function)]:
                (HERE/'programs'/f'{ids}.{ext}').write_text(text)
            if name in ['reverse','prefix']:
                local_cpp,local_py=local_sources(name,n,py)
                for ext,text in [('cpp',local_cpp),('py',local_py)]:
                    (HERE/'programs'/f'{ids}.local.{ext}').write_text(text)
            cases.append({'id':ids,'length':n,'parameters':params,'result':result,'checks':[{'args':a,'expected':oracle(name,a)} for a in checks]})
    ids='predicate';params=['i'];result='b'
    for ext,text in [('nil','1:b=b(a)\n1:b=a>0\n'),('py','def positive(a):\n    return a > 0\n\ndef program(a):\n    return positive(a)\n'),('cpp','#include <cstdint>\nbool positive(std::int64_t a) { return a>0; }\nbool program(std::int64_t a) { return positive(a); }\n')]:
        (HERE/'programs'/f'{ids}.{ext}').write_text(text)
    cases.append({'id':ids,'length':1,'parameters':params,'result':result,'checks':[{'args':[v],'expected':v>0} for v in [-7,0,1]]})
    (HERE/'cases.json').write_text(json.dumps({'schema':1,'cases':cases},indent=2)+'\n')

if __name__ == '__main__': corpus()
