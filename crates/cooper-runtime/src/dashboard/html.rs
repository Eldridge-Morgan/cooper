/// Render the complete dashboard as a self-contained HTML page.
pub fn render(api_port: u16) -> String {
    TEMPLATE.replace("__PORT__", &api_port.to_string())
}

const TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>cooper</title>
<script src="https://cdn.jsdelivr.net/npm/@dagrejs/dagre@1.1.4/dist/dagre.min.js"></script>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#fff;color:#000;font-family:"SF Mono","JetBrains Mono","Fira Code",monospace;font-size:12px;line-height:1.4}
.s{max-width:1100px;margin:0 auto;padding:10px 20px}
.h{display:flex;justify-content:space-between;align-items:center;padding:6px 0;border-bottom:1px solid #e0e0e0;margin-bottom:8px}
.h .l{font-weight:bold;font-size:13px}
.h .r{color:#aaa;font-size:11px}
.dot{display:inline-block;width:5px;height:5px;background:#000;margin-right:4px;animation:p 2s infinite}
@keyframes p{0%,100%{opacity:1}50%{opacity:.2}}
.tabs{display:flex;border-bottom:1px solid #e0e0e0;margin-bottom:8px}
.t{padding:5px 14px;cursor:pointer;color:#aaa;background:none;border:1px solid transparent;border-bottom:none;font:inherit;font-size:11px;position:relative;top:1px}
.t:hover{color:#000}.t.a{color:#000;border-color:#e0e0e0;background:#fff;border-bottom:1px solid #fff;font-weight:bold}
.P{display:none}.P.a{display:block}
#gc{width:100%;overflow-x:auto}
#gc svg{display:block;margin:0 auto}
table{width:100%;border-collapse:collapse;font-size:11px}
th{text-align:left;padding:4px 8px;border-bottom:1.5px solid #000;font-size:10px;text-transform:uppercase;letter-spacing:.6px}
td{padding:3px 8px;border-bottom:1px solid #f0f0f0}
tr:hover td{background:#fafafa}
.b{font-weight:bold}.d{color:#aaa}
.bg{background:#000;color:#fff;padding:0 5px;font-size:9px;font-weight:bold}
.ex{display:grid;grid-template-columns:1fr 1fr;gap:10px}
.ep{border:1px solid #e0e0e0;padding:8px}
.ep h3{font-size:9px;text-transform:uppercase;letter-spacing:.6px;color:#aaa;margin-bottom:6px;font-weight:bold}
select,input,textarea{background:#fff;color:#000;border:1px solid #ddd;padding:4px 6px;font:inherit;font-size:11px;width:100%;outline:none}
textarea{resize:vertical;min-height:50px}
button{cursor:pointer;background:#000;color:#fff;border:none;padding:5px 14px;font:inherit;font-size:11px;font-weight:bold;margin-top:6px}
.fl{margin-bottom:6px}
.fl label{display:block;font-size:9px;color:#aaa;margin-bottom:1px;text-transform:uppercase;letter-spacing:.4px}
pre.rs{background:#fafafa;border:1px solid #eee;padding:8px;white-space:pre-wrap;font-size:11px;min-height:80px;max-height:300px;overflow-y:auto}
.sc{font-size:11px;margin-bottom:4px;font-weight:bold}
.ls{font-size:11px;max-height:400px;overflow-y:auto}
.le{padding:2px 0;border-bottom:1px solid #f5f5f5;display:flex;gap:10px}
.lt{color:#ccc;min-width:60px}.lm{font-weight:bold;min-width:42px}.lp{color:#666;flex:1}
.lc{min-width:30px;text-align:right;font-weight:bold}.lc.g{color:#000}.lc.y{color:#bbb}
.ld{color:#ccc;min-width:44px;text-align:right}
.f{border-top:1px solid #e0e0e0;margin-top:10px;padding-top:6px;color:#ccc;font-size:10px;display:flex;justify-content:space-between}
</style>
</head>
<body>
<div class="s">
<div class="h">
  <div class="l" id="logo">cooper</div>
  <div class="r"><span class="dot"></span>localhost:__PORT__</div>
</div>
<div class="tabs">
  <button class="t a" data-p="map">Map</button>
  <button class="t" data-p="routes">Routes</button>
  <button class="t" data-p="api">Explorer</button>
  <button class="t" data-p="log">Log</button>
</div>
<div class="P a" id="map"><div id="gc"></div></div>
<div class="P" id="routes"><table><thead><tr><th>method</th><th>path</th><th>handler</th><th>source</th><th></th></tr></thead><tbody id="rt"></tbody></table></div>
<div class="P" id="api"><div class="ex"><div class="ep"><h3>Request</h3><div class="fl"><label>Route</label><select id="xr"></select></div><div class="fl"><label>Headers</label><input id="xh" placeholder="Authorization: Bearer ..."/></div><div class="fl"><label>Body</label><textarea id="xb" placeholder='{"name":"Alice"}'></textarea></div><button onclick="send()">Send</button></div><div class="ep"><h3>Response</h3><div class="sc" id="xs"></div><pre class="rs" id="xo">// send a request</pre></div></div></div>
<div class="P" id="log"><div class="ls" id="ll"><span style="color:#ccc">watching...</span></div></div>
<div class="f"><span>cooper v0.4.0</span><span id="ft"></span></div>
</div>
<script>
const API="http://localhost:__PORT__";

document.querySelectorAll(".t").forEach(t=>{t.onclick=()=>{
  document.querySelectorAll(".t").forEach(x=>x.classList.remove("a"));
  document.querySelectorAll(".P").forEach(x=>x.classList.remove("a"));
  t.classList.add("a");document.getElementById(t.dataset.p).classList.add("a");
}});

async function load(){
  try{
    const r=await fetch(API+"/_cooper/info");
    const D=await r.json();
    renderGraph(D);renderRoutes(D.routes);renderExplorer(D.routes);
    document.getElementById("ft").textContent=D.routes.length+" routes · "+D.topics.length+" topics · "+D.queues.length+" queues";
  }catch(e){
    document.getElementById("gc").innerHTML='<p style="color:#aaa;padding:20px">connecting to '+API+'...</p>';
    setTimeout(load,2000);
  }
}

function renderGraph(D){
  // Build nodes from analysis
  const svcs={};
  D.routes.forEach(r=>{
    const p=r.source.split("/");
    const svc=p.length>=2&&p[0]==="services"?p[1]:"root";
    if(!svcs[svc])svcs[svc]={pub:0,auth:0,dbs:[],crons:[]};
    if(r.auth)svcs[svc].auth++;else svcs[svc].pub++;
  });
  D.databases.forEach(d=>{
    for(const s in svcs){svcs[s].dbs.push(d.name);break;}
  });
  D.crons.forEach(c=>{
    const p=c.source_file?c.source_file.split("/"):[];
    const svc=p.length>=2&&p[0]==="services"?p[1]:Object.keys(svcs)[0];
    if(svcs[svc])svcs[svc].crons.push(c.name);
  });

  const g=new dagre.graphlib.Graph();
  g.setGraph({rankdir:"LR",nodesep:28,ranksep:55,marginx:16,marginy:16});
  g.setDefaultEdgeLabel(()=>({}));

  // Service nodes
  for(const[name,info]of Object.entries(svcs)){
    const lines=1+(info.dbs.length>0?1:0)+info.crons.length;
    g.setNode(name,{width:200,height:36+lines*13,type:"service",label:name,pub:info.pub,auth:info.auth,dbs:info.dbs,crons:info.crons});
  }

  // Topic nodes
  D.topics.forEach(t=>{
    g.setNode("t_"+t.name,{width:t.name.length*8+24,height:22,type:"topic",label:t.name});
  });

  // Queue nodes
  D.queues.forEach(q=>{
    g.setNode("q_"+q.name,{width:q.name.length*8+24,height:22,type:"queue",label:q.name});
  });

  // Edges: service → topic (publish)
  D.topics.forEach(t=>{
    const p=t.source_file?t.source_file.split("/"):[];
    const svc=p.length>=2&&p[0]==="services"?p[1]:null;
    if(svc&&svcs[svc])g.setEdge(svc,"t_"+t.name);
  });

  // Edges: topic → subscriber services (heuristic: other services)
  D.topics.forEach(t=>{
    const p=t.source_file?t.source_file.split("/"):[];
    const pub_svc=p.length>=2&&p[0]==="services"?p[1]:null;
    for(const svc of Object.keys(svcs)){
      if(svc!==pub_svc)g.setEdge("t_"+t.name,svc);
    }
  });

  // Edges: service → queue
  D.queues.forEach(q=>{
    const p=q.source_file?q.source_file.split("/"):[];
    const svc=p.length>=2&&p[0]==="services"?p[1]:null;
    if(svc&&svcs[svc])g.setEdge(svc,"q_"+q.name);
  });

  dagre.layout(g);

  // Render
  const gr=g.graph();
  const ns="http://www.w3.org/2000/svg";
  const svgW=gr.width+32,svgH=gr.height+50;
  const svg=document.createElementNS(ns,"svg");
  svg.setAttribute("viewBox","0 0 "+svgW+" "+svgH);
  svg.setAttribute("width",Math.min(svgW,1060));
  svg.style.fontFamily="monospace";

  // Arrowhead
  const defs=document.createElementNS(ns,"defs");
  const mk=document.createElementNS(ns,"marker");
  mk.setAttribute("id","a");mk.setAttribute("viewBox","0 0 8 8");
  mk.setAttribute("refX","8");mk.setAttribute("refY","4");
  mk.setAttribute("markerWidth","5");mk.setAttribute("markerHeight","5");
  mk.setAttribute("orient","auto-start-reverse");
  const mp=document.createElementNS(ns,"path");
  mp.setAttribute("d","M0,0 L8,4 L0,8");mp.setAttribute("fill","none");
  mp.setAttribute("stroke","#000");mp.setAttribute("stroke-width","1.2");
  mk.appendChild(mp);defs.appendChild(mk);svg.appendChild(defs);

  function el(tag,attrs,text){
    const e=document.createElementNS(ns,tag);
    for(const[k,v]of Object.entries(attrs||{}))e.setAttribute(k,v);
    if(text!==undefined)e.textContent=text;return e;
  }

  // Edges
  g.edges().forEach(e=>{
    const edge=g.edge(e);const pts=edge.points;
    let d="M"+pts[0].x+","+pts[0].y;
    for(let i=1;i<pts.length;i++)d+=" L"+pts[i].x+","+pts[i].y;
    const path=el("path",{d:d,fill:"none",stroke:"#000","stroke-width":"1","stroke-dasharray":"5,4","marker-end":"url(#a)"});
    const anim=document.createElementNS(ns,"animate");
    anim.setAttribute("attributeName","stroke-dashoffset");
    anim.setAttribute("from","0");anim.setAttribute("to","-18");
    anim.setAttribute("dur",(1+Math.random()*0.8).toFixed(1)+"s");
    anim.setAttribute("repeatCount","indefinite");
    path.appendChild(anim);svg.appendChild(path);
  });

  // Nodes
  g.nodes().forEach(nid=>{
    const n=g.node(nid);const x=n.x-n.width/2,y=n.y-n.height/2;
    if(n.type==="service"){
      svg.appendChild(el("rect",{x:x,y:y,width:n.width,height:n.height,fill:"#fff",stroke:"#000","stroke-width":"1.5"}));
      svg.appendChild(el("text",{x:x+14,y:y+18,"font-size":"13","font-weight":"bold"},n.label));
      svg.appendChild(el("text",{x:x+14,y:y+33,"font-size":"9",fill:"#aaa"},"→ "+n.pub+" public  "+n.auth+" auth"));
      let ly=46;
      if(n.dbs.length){svg.appendChild(el("text",{x:x+14,y:y+ly,"font-size":"8",fill:"#ccc"},"■ "+n.dbs.join(", ")));ly+=13;}
      n.crons.forEach(c=>{svg.appendChild(el("text",{x:x+14,y:y+ly,"font-size":"8",fill:"#ccc"},"◷ "+c));ly+=13;});
    }else{
      svg.appendChild(el("rect",{x:x,y:y,width:n.width,height:n.height,fill:"#000"}));
      svg.appendChild(el("text",{x:n.x,y:n.y+4,"text-anchor":"middle","font-size":"10","font-weight":"bold",fill:"#fff"},n.label));
    }
  });

  // Legend
  const ly=svgH-16;
  const lg=el("g",{transform:"translate(16,"+ly+")","font-size":"9",fill:"#bbb"});
  lg.appendChild(el("rect",{width:5,height:5,fill:"#000"}));lg.appendChild(el("text",{x:8,y:4},"Postgres"));
  lg.appendChild(el("rect",{x:70,width:5,height:5,fill:"#000"}));lg.appendChild(el("text",{x:78,y:4},"Valkey"));
  lg.appendChild(el("rect",{x:126,width:5,height:5,fill:"#000"}));lg.appendChild(el("text",{x:134,y:4},"NATS"));
  const fl=el("line",{x1:210,y1:2,x2:234,y2:2,stroke:"#000","stroke-width":"1","stroke-dasharray":"5,4"});lg.appendChild(fl);
  lg.appendChild(el("text",{x:240,y:4},"flow"));
  lg.appendChild(el("rect",{x:290,y:-2,width:8,height:8,fill:"#000"}));lg.appendChild(el("text",{x:302,y:4},"topic/queue"));
  lg.appendChild(el("rect",{x:390,y:-2,width:8,height:8,fill:"none",stroke:"#000","stroke-width":"1.5"}));lg.appendChild(el("text",{x:402,y:4},"service"));
  svg.appendChild(lg);

  document.getElementById("gc").innerHTML="";
  document.getElementById("gc").appendChild(svg);
}

function renderRoutes(routes){
  document.getElementById("rt").innerHTML=routes.map(r=>"<tr><td class=b>"+r.method+"</td><td>"+r.path+"</td><td class=d>"+r.handler+"</td><td class=d>"+r.source+"</td><td>"+(r.auth?"<span class=bg>AUTH</span>":"")+"</td></tr>").join("");
}

function renderExplorer(routes){
  document.getElementById("xr").innerHTML=routes.map(r=>"<option>"+r.method+" "+r.path+"</option>").join("");
}

async function send(){
  const[method,path]=document.getElementById("xr").value.split(" ");
  let url=API+path;
  const params=path.match(/:(\w+)/g);
  if(params)params.forEach(p=>{const v=prompt(p.slice(1)+"?","test");if(v)url=url.replace(p,v);});
  const hdrs={"Content-Type":"application/json"};
  const hv=document.getElementById("xh").value;
  if(hv)hv.split(",").forEach(h=>{const[k,...v]=h.split(":");if(k)hdrs[k.trim()]=v.join(":").trim();});
  const opts={method:method,headers:hdrs};
  const body=document.getElementById("xb").value;
  if(body&&method!=="GET")opts.body=body;
  const t0=performance.now();
  try{
    const res=await fetch(url,opts);
    const dur=Math.round(performance.now()-t0);
    const txt=await res.text();
    let fmt;try{fmt=JSON.stringify(JSON.parse(txt),null,2)}catch{fmt=txt}
    document.getElementById("xo").textContent=fmt;
    document.getElementById("xs").textContent=res.status+" "+res.statusText+" — "+dur+"ms";
    document.getElementById("xs").className="sc"+(res.ok?" ok":"");
    addLog(method,path,res.status,dur);
  }catch(e){
    document.getElementById("xo").textContent="// "+e.message;
    document.getElementById("xs").textContent="error";document.getElementById("xs").className="sc";
  }
}

function addLog(m,p,s,d){
  const ll=document.getElementById("ll");
  if(ll.querySelector("span"))ll.innerHTML="";
  const now=new Date().toLocaleTimeString("en-US",{hour12:false});
  ll.innerHTML+='<div class="le"><span class="lt">'+now+'</span><span class="lm">'+m+'</span><span class="lp">'+p+'</span><span class="lc '+(s<300?"g":"y")+'">'+s+'</span><span class="ld">'+d+'ms</span></div>';
  ll.scrollTop=ll.scrollHeight;
}

// Logo decrypt
(function(){
const T=["   ___                          ","  / __\\___   ___  _ __   ___ _ __ "," / /  / _ \\ / _ \\| '_ \\ / _ \\ '__|","/ /__| (_) | (_) | |_) |  __/ |   ","\\____/\\___/ \\___/| .__/ \\___|_|   ","                  |_|             "];
const C="@#$%&*+=~?/<>[]{}|^";const el=document.getElementById("logo");
el.style.cssText="font-size:9px;line-height:1.1;white-space:pre;font-weight:normal";
const W=Math.max(...T.map(l=>l.length));
const g=T.map(l=>l.padEnd(W).split("").map(c=>c===" "?" ":C[Math.floor(Math.random()*C.length)]));
const render=()=>{el.textContent=g.map(r=>r.join("")).join("\n")};
let n=0;const si=setInterval(()=>{g.forEach((r,i)=>r.forEach((c,j)=>{if(T[i][j]!==" ")g[i][j]=C[Math.floor(Math.random()*C.length)]}));render();if(++n>12){clearInterval(si);let col=0;const ri=setInterval(()=>{for(let k=0;k<3&&col<W;k++,col++)g.forEach((r,i)=>{if(col<T[i].length)g[i][col]=T[i][col]});g.forEach((r,i)=>r.forEach((c,j)=>{if(j>col&&T[i][j]!==" ")g[i][j]=C[Math.floor(Math.random()*C.length)]}));render();if(col>=W){clearInterval(ri);el.textContent=T.join("\n");setInterval(()=>{const ns=[];for(let i=0;i<3;i++){const ci=Math.floor(Math.random()*T.length),cj=Math.floor(Math.random()*W);if(T[ci][cj]!==" ")ns.push([ci,cj])}ns.forEach(([i,j])=>g[i][j]=C[Math.floor(Math.random()*C.length)]);render();setTimeout(()=>{ns.forEach(([i,j])=>g[i][j]=T[i][j]);render()},60)},5000)}},20)}},35);render()
})();

setInterval(async()=>{try{const r=await fetch(API+"/_cooper/health");if(!r.ok)throw 0}catch{}},5000);

load();
</script>
</body>
</html>"##;
