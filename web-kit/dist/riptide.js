(()=>{"use strict";try{if(!self.WebGL2RenderingContext||!self.Promise||!self.PointerEvent||!self.matchMedia||matchMedia("(prefers-reduced-motion: reduce)").matches||navigator.connection&&navigator.connection.saveData)return}catch{return}const m=document.currentScript;if(!m||!m.src)return;const g=new URL(".",m.src),v=252,p=6,x=8*1024*1024,d=2,T=1e4,_=250,A=`#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`,R=`#version 300 es
precision highp float;

const int WAKE_SLOTS = ${p};
const float TILE_PITCH = ${v}.0;
const float WAVE_V = 320.0;
const float WAVE_SIGMA = 14.0;
const float WAVE_DAMP = 4.8;
const float WAVE_SPREAD = 480.0;
const float REFRACT_PX = 2.0;
const float IOR_SPREAD = 0.68;

uniform sampler2D floor_tex;
uniform vec2 viewport;
uniform vec2 origin;
uniform float density;
uniform float tide;
uniform vec4 wakes[WAKE_SLOTS];
out vec4 color;

vec2 touch_flow(vec2 px, vec4 touch) {
  float age = tide - touch.z;
  if (touch.w <= 0.0 || age < 0.0) return vec2(0.0);

  vec2 ray = px - touch.xy;
  float travel = WAVE_V * age;
  float reach = 4.0 * WAVE_SIGMA + 0.05 * travel;
  float square = max(abs(ray.x), abs(ray.y));
  if (square > travel + reach) return vec2(0.0);
  if (travel > reach && square < (travel - reach) * 0.70710678) return vec2(0.0);

  float d = length(ray);
  if (abs(d - travel) > reach) return vec2(0.0);
  float a = touch.w * exp(-age / WAVE_DAMP) / sqrt(1.0 + d / WAVE_SPREAD);
  float s = (d - travel) / WAVE_SIGMA;
  return ray / max(d, 1e-3) * (a * s * exp(-0.5 * s * s));
}

vec3 submerged(vec2 px, vec2 flow) {
  vec2 g = flow * REFRACT_PX;
  vec2 r = (px + g * (1.0 - IOR_SPREAD)) / TILE_PITCH;
  vec2 m = (px + g) / TILE_PITCH;
  vec2 b = (px + g * (1.0 + IOR_SPREAD)) / TILE_PITCH;
  return vec3(texture(floor_tex, r).r, texture(floor_tex, m).g, texture(floor_tex, b).b);
}

void main() {
  vec2 screen_px = vec2(gl_FragCoord.x, viewport.y - gl_FragCoord.y) / density;
  vec2 document_px = screen_px + origin;
  vec2 flow = vec2(0.0);
  for (int i = 0; i < WAKE_SLOTS; ++i) flow += touch_flow(document_px, wakes[i]);
  color = vec4(submerged(document_px, flow), 1.0);
}`;class u{static R(){const s=Math.max(1,Math.ceil(devicePixelRatio));if(!isFinite(s)||s>3)return null;const i=document.createElement("canvas");i.className="riptide",i.setAttribute("aria-hidden","true"),i.style.cssText="position:absolute;z-index:0;opacity:0;pointer-events:none",i.width=i.height=1;const r=a=>a.preventDefault();i.addEventListener("webglcontextcreationerror",r);const t=i.getContext("webgl2",{alpha:!0,antialias:!1,depth:!1,desynchronized:!0,failIfMajorPerformanceCaveat:!0,powerPreference:"low-power",premultipliedAlpha:!1,preserveDrawingBuffer:!1,stencil:!1});if(i.removeEventListener("webglcontextcreationerror",r),!t)return null;const o=new Image;return new Promise((a,e)=>{o.onload=()=>a(o),o.onerror=e,o.src=new URL(s===1?"brass-tiles.png":`brass-tiles@${s}x.png`,g).href}).then(a=>{const e=t.createProgram(),n=[[t.VERTEX_SHADER,A],[t.FRAGMENT_SHADER,R]];for(let f=0;f<n.length;++f){const l=t.createShader(n[f][0]);if(t.shaderSource(l,n[f][1]),t.compileShader(l),!t.getShaderParameter(l,t.COMPILE_STATUS))throw 0;t.attachShader(e,l),t.deleteShader(l)}if(t.linkProgram(e),!t.getProgramParameter(e,t.LINK_STATUS))throw 0;t.useProgram(e),t.bindVertexArray(t.createVertexArray()),t.activeTexture(t.TEXTURE0),t.bindTexture(t.TEXTURE_2D,t.createTexture()),t.texParameteri(t.TEXTURE_2D,t.TEXTURE_MIN_FILTER,t.LINEAR),t.texParameteri(t.TEXTURE_2D,t.TEXTURE_MAG_FILTER,t.LINEAR),t.texParameteri(t.TEXTURE_2D,t.TEXTURE_WRAP_S,t.REPEAT),t.texParameteri(t.TEXTURE_2D,t.TEXTURE_WRAP_T,t.REPEAT),t.texImage2D(t.TEXTURE_2D,0,t.RGBA,t.RGBA,t.UNSIGNED_BYTE,a);const c=f=>t.getUniformLocation(e,f);return t.uniform1i(c("floor_tex"),0),t.clearColor(0,0,0,0),new u(i,t,s,[c("density"),c("origin"),c("tide"),c("viewport"),c("wakes[0]")])}).catch(()=>(u.g(t),null))}static g(s){try{const i=s.getExtension("WEBGL_lose_context");i&&i.loseContext()}catch{}}constructor(s,i,r,t){this.e=s,this.t=i,this.y=r,this.c=t,this.r=new Float32Array(p*4),this.w=0,this.o=0,this.n=null,this.u=[],this.d=null,this.v=null,this.m=0,this.a=!1,this.l=[]}bind(){const s=(e,n)=>this.f(e.clientX+scrollX,e.clientY+scrollY,n*d),i=e=>e instanceof Element?e.closest(".poolrooms-frame,.plate")||e.closest("a,button"):null;this.i(self,"pointerdown",e=>s(e,1.6),{passive:!0}),this.i(self,"pointermove",e=>this.M(e),{passive:!0}),this.i(self,"pointerover",e=>{const n=i(e.target);n&&!n.contains(e.relatedTarget)&&s(e,.9)},{passive:!0}),this.i(self,"pointerout",e=>{const n=i(e.target);n&&!n.contains(e.relatedTarget)&&s(e,.42)},{passive:!0}),this.i(self,"focusin",e=>{const n=i(e.target);if(!n)return;const c=n.getBoundingClientRect();this.f(c.left+c.width/2+scrollX,c.top+c.height/2+scrollY,.9*d)});const r=()=>{this.n=null,(this.e.parentNode||this.o)&&this.h("ready")};this.i(self,"scroll",r,{passive:!0}),this.i(self,"resize",r,{passive:!0}),this.i(document,"visibilitychange",()=>{document.hidden&&this.h("ready")});const t=matchMedia("(prefers-reduced-motion: reduce)"),o=e=>{e.matches&&this.s()};t.addEventListener?this.i(t,"change",o):(t.addListener(o),this.l.push(()=>t.removeListener(o)));const a=navigator.connection;a&&a.addEventListener&&this.i(a,"change",()=>{a.saveData&&this.s()}),this.i(self,"pagehide",e=>{e.persisted||this.s()}),this.i(this.e,"webglcontextlost",()=>this.s()),document.body.dataset.riptideRain==="passing"&&(this.v=[0,0,0,0,0].map(()=>Math.random()*Math.PI*2),this.m=setTimeout(()=>this.x(),Math.max(0,T-performance.now()))),document.documentElement.setAttribute("data-riptide","ready")}i(s,i,r,t){const o=a=>{if(!this.a)try{r(a)}catch{this.s()}};s.addEventListener(i,o,t),this.l.push(()=>s.removeEventListener(i,o,t))}M(s){if(s.pointerType!=="mouse")return;const i=performance.now(),r=[s.clientX+scrollX,s.clientY+scrollY,i];if(!this.n){this.n=r;return}const t=this.n[0],o=this.n[1],a=this.n[2],e=Math.hypot(r[0]-t,r[1]-o);e<54||i-a<72||(this.n=r,this.f(r[0],r[1],Math.min(.72,.2+e/Math.max(i-a,1)*.15)*d))}x(){if(!this.a)try{const s=performance.now()/1e3,i=this.v,r=(Math.sin(s/29+i[0])+Math.sin(s/43+i[1])+Math.sin(s/71+i[2]))/3,t=Math.min(1,Math.max(0,(r-.54)/.46));t&&!document.hidden&&Math.random()<t&&this.P(s,i,t),this.m=setTimeout(()=>this.x(),_)}catch{this.s()}}P(s,i,r){const t=()=>Math.random()+Math.random()-1,o=.5+.42*Math.sin(s/53+i[3])+.24*t(),a=.5+.42*Math.sin(s/67+i[4])+.24*t();this.f(o*innerWidth+scrollX,a*innerHeight+scrollY,(.35+.45*r)*d)}f(s,i,r){if(this.a)return;if(!this.e.parentNode&&!this.S()){this.s();return}const t=this.w++%p*4;this.r.set([s,i,performance.now()/1e3,r],t),this.T()}S(){const s=innerWidth,i=innerHeight,r=devicePixelRatio;if(!isFinite(r)||s<=0||i<=0||r<=0||r>this.y)return!1;const t=Math.ceil(s*r),o=Math.ceil(i*r),a=this.t.getParameter(this.t.MAX_VIEWPORT_DIMS),e=this.t.getParameter(this.t.MAX_RENDERBUFFER_SIZE);if(t*o>x||t>e||o>e||t>a[0]||o>a[1])return!1;const n=scrollX,c=scrollY;return this.d=[r,n,c],this.e.style.left=`${n}px`,this.e.style.top=`${c}px`,this.e.style.width=`${s}px`,this.e.style.height=`${i}px`,this.e.width=t,this.e.height=o,this.t.drawingBufferWidth!==t||this.t.drawingBufferHeight!==o?!1:(this.t.viewport(0,0,t,o),this.I(s,i,r),document.body.prepend(this.e),!0)}T(){!this.a&&!document.hidden&&!this.o&&(this.o=requestAnimationFrame(s=>{try{this.b(s)}catch{this.s()}}))}h(s){this.o&&cancelAnimationFrame(this.o),this.o=0,this.r.fill(0),this.e.style.opacity=0,this.e.remove(),this.e.width=this.e.height=1,this.d=null,s?document.documentElement.setAttribute("data-riptide",s):document.documentElement.removeAttribute("data-riptide")}s(){if(!this.a){for(this.a=!0,clearTimeout(this.m);this.l.length;)try{this.l.pop()()}catch{}u.g(this.t);try{this.h(null)}catch{}}}I(s,i,r){const t=Array.prototype.map.call(document.querySelectorAll(".poolrooms-frame,.plate"),e=>{const n=e.getBoundingClientRect();return{_:Math.max(0,n.left),p:Math.max(0,n.top),A:Math.min(s,n.right),E:Math.min(i,n.bottom)}}).filter(e=>e._<e.A&&e.p<e.E),o=[0,i];for(let e=0;e<t.length;++e)o.push(t[e].p,t[e].E);o.sort((e,n)=>e-n);for(let e=o.length-1;e>0;--e)o[e]===o[e-1]&&o.splice(e,1);const a=[];for(let e=1;e<o.length;++e){const n=o[e-1],c=o[e],f=t.filter(h=>h.p<c&&h.E>n).map(h=>[h._,h.A]).sort((h,y)=>h[0]-y[0]);let l=0;for(let h=0;h<f.length;++h)f[h][0]>l&&a.push([l,n,f[h][0],c]),l=Math.max(l,f[h][1]);l<s&&a.push([l,n,s,c])}this.u=a.map(e=>{const n=Math.floor(e[0]*r),c=Math.ceil(e[2]*r),f=Math.floor((i-e[3])*r),l=Math.ceil((i-e[1])*r);return[n,f,c-n,l-f]})}b(s){this.o=0;const[i,r,t]=this.d,o=innerWidth,a=innerHeight,e=this.e.width,n=this.e.height;if(devicePixelRatio!==i||e!==Math.ceil(o*i)||n!==Math.ceil(a*i)){this.h("ready");return}const c=s/1e3,f=(Math.hypot(o,a)+56)/320;let l=!1;for(let h=2;h<this.r.length;h+=4)this.r[h+1]>0&&c-this.r[h]<f?l=!0:this.r[h+1]=0;this.t.uniform1f(this.c[0],i),this.t.uniform2f(this.c[1],r,t),this.t.uniform1f(this.c[2],c),this.t.uniform2f(this.c[3],e,n),this.t.uniform4fv(this.c[4],this.r),this.t.disable(this.t.SCISSOR_TEST),this.t.clear(this.t.COLOR_BUFFER_BIT),this.t.enable(this.t.SCISSOR_TEST);for(let h=0;h<this.u.length;++h)this.t.scissor(...this.u[h]),this.t.drawArrays(this.t.TRIANGLES,0,3);this.e.style.opacity=1,document.documentElement.setAttribute("data-riptide","live"),l?this.T():this.h("ready")}}Promise.resolve().then(()=>u.R()).then(E=>{if(E)try{E.bind()}catch{E.s()}},()=>{})})();
