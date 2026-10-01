"""Extra array kernels/sizes not used to develop the initial storage lowering."""
import json
from pathlib import Path
import generate

HERE=Path(__file__).resolve().parent


def main():
    cases=[]
    for n in [16,64,192]:
        arr=f'std::array<std::int64_t,{n}>'
        for name in ['map','conditional','swap_pairs']:
            nil={
                'map':f'({n}):{n}=@(a,0;b<#a;a[b:a[b]*3+1],b+1;a)\n',
                'conditional':f'({n}):{n}=@(a,0;b<#a;a[b]>0?a[b:a[b]+1]:a,b+1;a)\n',
                'swap_pairs':f'({n}):{n}=@(a,0;b<#a;a[b:a[b+1]][b+1:a[b]],b+2;a)\n',
            }[name]
            body={
                'map':f'for(std::int64_t i=0;i<{n};i++) a[i]=get(a,i)*3+1; return a;',
                'conditional':f'for(std::int64_t i=0;i<{n};i++) if(get(a,i)>0) a[i]=get(a,i)+1; return a;',
                'swap_pairs':f'for(std::int64_t i=0;i<{n};i+=2) {{ auto old=get(a,i); a[i]=get(a,i+1); a[i+1]=old; }} return a;',
            }[name]
            function=f'{arr} program({arr} a) {{\n    {body}\n}}\n'
            cpp='#include <array>\n#include <cstdint>\n#include <cstdlib>\n'+generate.GET+function
            py={
                'map':'def program(a):\n    result = a.copy()\n    for i in range(len(a)):\n        result[i] = result[i]*3+1\n    return result\n',
                'conditional':'def program(a):\n    result = a.copy()\n    for i in range(len(a)):\n        if result[i] > 0:\n            result[i] += 1\n    return result\n',
                'swap_pairs':'def program(a):\n    result = a.copy()\n    for i in range(0, len(a), 2):\n        result[i], result[i+1] = result[i+1], result[i]\n    return result\n',
            }[name]
            checks=[]
            for values in [[i%7-3 for i in range(n)],[0]*n,[7]*n]:
                expected=([x*3+1 for x in values] if name=='map' else [x+1 if x>0 else x for x in values] if name=='conditional' else [values[i^1] for i in range(n)])
                checks.append({'args':[values],'expected':expected})
            ids=f'{name}-{n}'
            for ext,text in [('nil',nil),('cpp',cpp),('body.cpp',function),('py',py)]:
                (HERE/'programs'/f'{ids}.{ext}').write_text(text)
            cases.append({'id':ids,'length':n,'parameters':[n],'result':n,'checks':checks})
    (HERE/'heldout.json').write_text(json.dumps({'schema':1,'cases':cases},indent=2)+'\n')

if __name__=='__main__':main()
