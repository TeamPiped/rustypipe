// This code is used to test the deobfuscation javascript extraction.
// Since YouTube's player code is copyrighted, I can just copy-paste it
// into my project.

/*
The filler javascript code comes from everything.js
Source: https://github.com/michaelficarra/everything.js
Copyright (c) 2014, Michael Ficarra, BSD 3-Clause License
*/

tab:for(;;)break	tab;
verticalTab:for(;;)breakverticalTab;
formFeed:for(;;)breakformFeed;
space:for(;;)break space;
nbsp:for(;;)break nbsp;
bom:for(;;)break﻿bom;

lineFeed:0
0;
carriageReturn:00;
carriageReturnLineFeed:0
0;
lineSeparator:00;
paragraphSeparator:00;

var $, _, \u0078, x$, x_, x\u0030, xa, x0, x0a, x0123456789,
  qwertyuiopasdfghjklzxcvbnm, QWERTYUIOPASDFGHJKLZXCVBNM;
var œ一, ǻ둘, ɤ〩, φ, ﬁⅷ, ユニコード, x‌‍;

null; true; false;

0; 00; 1234567890; 01234567;
0.; 0.00; 10.00; .0; .00
0e0; 0E0; 0.e0; 0.00e+0; .00e-0;
0x0; 0X0; 0x0123456789abcdefABCDEF;
2e308;

""; "'"; "\'\"\\\b\f\n\r\t\v\0";
"\1\00\400\000";
"\x01\x23\x45\x67\x89\xAB\xCD\xEF";
"\u0123\u4567\u89AB\uCDEF"; "\
";

''; '"'; '\'\"\\\b\f\n\r\t\v\0';
'\1\00\400\000';
'\x01\x23\x45\x67\x89\xAB\xCD\xEF';
'\u0123\u4567\u89AB\uCDEF'; '\
';

/x/; /|/; /|||/;
/^$\b\B/; /(?=(?!(?:(.))))/;
/a.\f\n\r\t\v\0\[\-\/\\\x00\u0000/; /\d\D\s\S\w\W/;
/\ca\cb\cc\cd\ce\cf\cg\ch\ci\cj\ck\cl\cm\cn\co\cp\cq\cr\cs\ct\cu\cv\cw\cx\cy\cz/;
/\cA\cB\cC\cD\cE\cF\cG\cH\cI\cJ\cK\cL\cM\cN\cO\cP\cQ\cR\cS\cT\cU\cV\cW\cX\cY\cZ/;
/[a-z-]/; /[^\b\-^]/; /[/\]\\]/;
/./i; /./g; /./m; /./igm;
/.*/; /.*?/; /.+/; /.+?/; /.?/; /.??/;
/.{0}/; /.{0,}/; /.{0,0}/;

// STS
{signatureTimestamp:19187};

this;

x;

[]; [,]; [0]; [0,]; [,0]; [0,0]; [0,0,]; [0,,0]; [,,];

({}); ({x:0}); ({x:0,y:0}); ({x:0,}); ({'x':0,"y":0,var:0,});
({0:0}); ({0.:0}); ({0.0:0}); ({.0:0}); ({0e0:0}); ({0x0:0});
({
  get x(){}, set x(a){}, get 'y'(){}, set "y"(a){},
  get 0(){}, set 0(a){}, get var(){}, set var(x){},
});

0..a;

0[0];

x = function f(){ return f; }; x[0] = x; x.a = x;

new x(); new new x()();
new x[0](); new x.a(); new x[0].a(); new x.a[0]();
new x; new new x; new new x();
new new x().a; new new x()[0];

x(); x()(); x(x); x(x, x);
x.a().a(); x[0]()[0](); x().a[0]();

x++; x--;

delete void typeof+-~!x; ++x; --x;

0*0; 0/0; 0%0;

0+0; 0-0;

0<<0; 0>>0; 0>>>0;

0<0; 0>0; 0<=0; 0>=0;
0 instanceof function(){};
0 in{};

0==0; 0!=0; 0===0; 0!==0;

0&0; 0^0; 0|0; 0&&0; 0||0;

0?0:0; 0?0?0:0:0; 0||0?x=0:x=0;

x=0; x*=0; x/=0; x%=0; x+=0; x-=0;
x<<=0; x>>=0; x>>>=0; x&=0; x^=0; x|=0;

0,0; 0,0,0; x=0,x=0;

// Sig deobf heloper object

var qB={w8:function(a){a.reverse()},
EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},
Np:function(a,b){a.splice(0,b)}};

{} {;} {0} {0;} {0;0} {0;0;}

var x; var x,y; var x,y,z;
var x=0; var x=0,y; var x,y=0; var x=0,y=0;

;

if(0); if(0);else;

do;while(0);
while(0);
for(;;)break; for(0;0;0); for((0 in[]);0;);
for(var a;;)break; for(var a,b;0;0);
for(var a=0;;)break; for(var a=(0 in[]);0;);
for(x in{}); for(var x in{});
for(var x=[]in{}); for(var x=(0 in[])in{});

debugger;

// Sig deobf function
var Rva;
Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};

for(;0;)continue; x:for(;0;)continue x;

for(;;)break; x:for(;;)break x;
switch(0){case 0:break;}

function f(){ return; }
function f(){ return 0; }

with(0);

// n_deobf function
{p.X&&(b=a.get("n"))&&(b=aF[0](c),a.set("n",b),vZ.length||Vo(""))};

switch(0){} switch(0){case 0:} switch(0){case 0:case 0:}
switch(0){default:} switch(0){case 0:default:case 0:}
switch(0){case 0:;} switch(0){case 0:;;}
switch(0){default:;} switch(0){default:;;}

x:; x:y:;

// n_deobf array_str
var aF=[Vo];

try { throw 0; }catch(x){}

try{}catch(x){}
try{}finally{}
try{}catch(x){}finally{}

// nsig deobf function
Vo=function(a){var b=a.split(""),c=[function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(""))},
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

function f(){}
function f(x){}
function f(x,y){}
function f(){ function f(){} }

function f(){ "use strict" }
function f(){ 'use strict' }
function f(){ "other directive" }
function f(){ 'other directive' }
function f(){ ("string") }
function f(){ ('string') }
function f(){
  'string'
  +0
}

(function(){});
(function(x){});
(function(x,y){});
(function(){ function f(){} });
(function f(){});
(function f(x){});
(function f(x,y){});
(function f(){ function f(){} });
