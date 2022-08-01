https://rr3---sn-h0jeened.googlevideo.com/videoplayback?expire=1658232063&ei=n0jWYuCAFIz3gAeAx4nIAw&ip=93.235.185.61&id=o-AHnSNPNCkequX39D-ysNUiDKYmbe-a8EplrOAV2LQylr&itag=18&source=youtube&requiressl=yes&mh=a7&mm=31%2C29&mn=sn-h0jeened%2Csn-h0jelnez&ms=au%2Crdu&mv=m&mvi=3&pl=26&initcwndbps=1416250&spc=lT-KhsYr92Phls7wH9GQiLWRR-MGnTE&vprv=1&mime=video%2Fmp4&ns=AMUzTf9OiCSKRVVVRqr1VqMH&gir=yes&clen=17923723&ratebypass=yes&dur=208.027&lmt=1641514704547595&mt=1658209972&fvip=4&fexp=24001373%2C24007246&beids=23886220&c=WEB&txp=4538322&n=BI_n4PxQ22is-KKajKUW&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cratebypass%2Cdur%2Clmt&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&lsig=AG3C_xAwRgIhAOxJLTqKCFUwInEHtxtsH13V0i_fDws_vgCuilecqHa6AiEAhHMFv4WqPrFNZvxsBx3ee5GdVw_7_hMu0yebsClRfw8%3D&sig=AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f&cpn=uGaaNCVq9tAJ9K8j

# Signature `(s => sig)`

deobfuscationFunctionName = Rva
functionPattern = (Rva=function\([a-zA-Z0-9_]+\)\{.+?\})
deobfuscateFunction = var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};
helperObjectName = qB
helperPattern = (var qB=\{.+?\}\};)
helperObject = var qB={w8:function(a){a.reverse()},EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},Np:function(a,b){a.splice(0,b)}};
callerFunction = function deobfuscate(a){return Rva(a);}

cachedDeobfuscationCode = var qB={w8:function(a){a.reverse()},EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},Np:function(a,b){a.splice(0,b)}};var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};function deobfuscate(a){return Rva(a);}


Result:
obfuscatedSig = GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i
result = AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f


# Nsig `(n => n)`

FunctionName = Vo
Function = Vo=function(a){var b=a.split(""),c=[function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(""))},
928409064,-595856984,1403221911,653089124,-168714481,-1883008765,158931990,1346921902,361518508,1403221911,-362174697,-233641452,function(){for(var d=64,e=[];++d-e.length-32;){switch(d){case 91:d=44;continue;case 123:d=65;break;case 65:d-=18;continue;case 58:d=96;continue;case 46:d=95}e.push(String.fromCharCode(d))}return e},
b,158931990,791141857,-907319795,-1776185924,1595027902,-829736173,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},
-1274951142,function(){for(var d=64,e=[];++d-e.length-32;){switch(d){case 91:d=44;continue;case 123:d=65;break;case 65:d-=18;continue;case 58:d=96;continue;case 46:d=95}e.push(String.fromCharCode(d))}return e},
1758743891,function(d){d.reverse()},
-830417133,"AF43j",1942017693,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},
null,-959991459,-287691724,-1365731946,b,1250397544,-1883008765,-1912322658,b,1300441121,null,-1962382380,1954679120,function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},
-985125467,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},
null,497372841,-1912651541,function(d,e){d.push(e)},
function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},
function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f}];
c[30]=c;c[40]=c;c[46]=c;try{c[43](c[34]),c[45](c[40],c[47]),c[46](c[51],c[33]),c[16](c[47],c[36]),c[38](c[31],c[49]),c[16](c[11],c[39]),c[0](c[11]),c[35](c[0],c[30]),c[35](c[4],c[17]),c[34](c[48],c[7],c[11]()),c[35](c[4],c[23]),c[35](c[4],c[9]),c[5](c[48],c[28]),c[36](c[46],c[16]),c[4](c[41],c[1]),c[4](c[16],c[28]),c[3](c[40],c[17]),c[9](c[8],c[23]),c[45](c[30],c[4]),c[50](c[3],c[28]),c[36](c[51],c[23]),c[14](c[0],c[24]),c[14](c[35],c[1]),c[20](c[51],c[41]),c[15](c[8],c[0]),c[31](c[35]),c[29](c[26]),
c[36](c[8],c[32]),c[20](c[25],c[10]),c[2](c[22],c[8]),c[32](c[20],c[16]),c[32](c[47],c[49]),c[1](c[44],c[28]),c[39](c[16]),c[32](c[42],c[22]),c[46](c[14],c[48]),c[26](c[29],c[10]),c[46](c[9],c[3]),c[32](c[45])}catch(d){return"enhanced_except_85UBjOr-_w8_"+a}return b.join("")};

OldNParam = BI_n4PxQ22is-KKajKUW
DecryptedNParam = nrkec0fwgTWolw


https://rr3---sn-h0jeened.googlevideo.com/videoplayback?expire=1658232063&ei=n0jWYuCAFIz3gAeAx4nIAw&ip=93.235.185.61&id=o-AHnSNPNCkequX39D-ysNUiDKYmbe-a8EplrOAV2LQylr&itag=18&source=youtube&requiressl=yes&mh=a7&mm=31%2C29&mn=sn-h0jeened%2Csn-h0jelnez&ms=au%2Crdu&mv=m&mvi=3&pl=26&initcwndbps=1416250&spc=lT-KhsYr92Phls7wH9GQiLWRR-MGnTE&vprv=1&mime=video%2Fmp4&ns=AMUzTf9OiCSKRVVVRqr1VqMH&gir=yes&clen=17923723&ratebypass=yes&dur=208.027&lmt=1641514704547595&mt=1658209972&fvip=4&fexp=24001373%2C24007246&beids=23886220&c=WEB&txp=4538322&n=nrkec0fwgTWolw&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cratebypass%2Cdur%2Clmt&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&lsig=AG3C_xAwRgIhAOxJLTqKCFUwInEHtxtsH13V0i_fDws_vgCuilecqHa6AiEAhHMFv4WqPrFNZvxsBx3ee5GdVw_7_hMu0yebsClRfw8%3D&sig=AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f&cpn=uGaaNCVq9tAJ9K8j
