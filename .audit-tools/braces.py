import sys
def depth_of(s):
    i=0;n=len(s);d=0
    while i<n:
        c=s[i]
        if c=='/' and i+1<n and s[i+1]=='/':
            while i<n and s[i]!='\n': i+=1
            continue
        if c=='/' and i+1<n and s[i+1]=='*':
            i+=2
            while i+1<n and not (s[i]=='*' and s[i+1]=='/'): i+=1
            i+=2; continue
        if c=="'" and i+2<n:
            if s[i+1]=='\\':
                j=i+2
                while j<n and s[j]!="'": j+=1
                if j<n and j-i<=6: i=j+1; continue
            elif s[i+2]=="'": i+=3; continue
        if c=='"':
            i+=1
            while i<n:
                if s[i]=='\\': i+=2; continue
                if s[i]=='"': break
                i+=1
            i+=1; continue
        if c=='{': d+=1
        elif c=='}': d-=1
        i+=1
    return d
bad=0
for f in sys.argv[1:]:
    d=depth_of(open(f).read())
    if d!=0: print(' BAD  '+f+f'  (depth {d})'); bad=1
sys.exit(bad)
