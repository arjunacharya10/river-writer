import {render} from "solid-js/web";
import {createSignal,onMount,onCleanup} from "solid-js";
import {EditorState,Compartment} from "@codemirror/state";
import {EditorView,keymap,drawSelection,highlightActiveLine,rectangularSelection} from "@codemirror/view";
import {history,historyKeymap,defaultKeymap,indentWithTab,undo,redo} from "@codemirror/commands";
import {searchKeymap,highlightSelectionMatches} from "@codemirror/search";
import {bracketMatching} from "@codemirror/language";
import {showMinimap} from "@replit/codemirror-minimap";
import {scriptLook,shortScript,prose,longScript,classify} from "./script";
import "./style.css";

declare global {
  interface Window {
    trial: {view:EditorView; runChecks:()=>Promise<unknown>};
    __TAURI__?: {core:{invoke:(name:string,args:unknown)=>Promise<void>}};
  }
}
const frame=()=>new Promise<number>(resolve=>requestAnimationFrame(resolve));
const quantile=(xs:number[],q:number)=>[...xs].sort((a,b)=>a-b)[Math.floor((xs.length-1)*q)];
function App() {
  let host!:HTMLDivElement;
  let view:EditorView;
  const mode=new Compartment();
  const [theme,setTheme]=createSignal(matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  const [manuscript,setManuscript]=createSignal(false);
  const [words,setWords]=createSignal(0);
  const [status,setStatus]=createSignal("Ready");
  const count=(text:string)=>text.match(/\S+/g)?.length??0;
  function state(text:string) {
    return EditorState.create({doc:text,extensions:[
      history(),drawSelection(),highlightActiveLine(),rectangularSelection(),
      EditorView.lineWrapping,bracketMatching(),highlightSelectionMatches(),
      keymap.of([...defaultKeymap,...historyKeymap,...searchKeymap,indentWithTab,
        {key:"Mod-p",run:()=>{setStatus("Trial: Quick Open is not implemented");return true}},
        {key:"Mod-Shift-p",run:()=>{setStatus("Trial: command palette is not implemented");return true}}]),
      mode.of(scriptLook(manuscript())),
      showMinimap.of({create:()=>({dom:document.createElement("div")}),displayText:"blocks",showOverlay:"always"}),
      EditorView.updateListener.of(update=>{if(update.docChanged) setWords(count(update.state.doc.toString()))}),
      EditorView.contentAttributes.of({"aria-label":"Story editor","spellcheck":"false"}),
    ]});
  }
  function load(text:string) {view.setState(state(text));setWords(count(text));view.scrollDOM.scrollTop=0;}
  async function runChecks() {
    setStatus("Checking long-document typing and scrolling…");
    const original=longScript();
    setManuscript(false);load(original);await frame();await frame();
    const assert=(ok:boolean,message:string)=>{if(!ok)throw new Error(message)};
    assert(classify(shortScript)[4]==="cue","cue classification");
    assert(classify(shortScript)[5]==="parenthetical","parenthetical classification");
    assert(classify(shortScript)[6]==="dialogue","dialogue classification");
    assert(view.state.doc.toString()===original,"styling modified source");
    const first=document.querySelector<HTMLElement>('[data-kind="dialogue"]')!;
    const action=document.querySelector<HTMLElement>('[data-kind="action"]')!;
    const indent=first.getBoundingClientRect().left-action.getBoundingClientRect().left;
    assert(Math.abs(indent - (178.992-108)*4/3)<1,"dialogue indent differs from template");
    assert(Math.abs(action.getBoundingClientRect().width-432*4/3)<1,"action width differs from template");
    assert(Math.abs(first.getBoundingClientRect().width-248.976*4/3)<1,"dialogue width differs from template");
    const before=view.state.doc.length;
    view.dispatch({selection:{anchor:before}});
    view.dispatch({changes:{from:before,insert:"River"}});
    assert(undo(view),"undo failed");assert(view.state.doc.toString()===original,"undo changed source");
    assert(redo(view),"redo failed");assert(view.state.doc.toString().endsWith("River"),"redo failed");
    load(original);
    const typing:number[]=[],frames:number[]=[];
    let previous=await frame();
    for(let i=0;i<60;i++) {
      const now=await frame();frames.push(now-previous);previous=now;
      const start=performance.now();
      const at=view.state.doc.line(7).to;
      view.dispatch({changes:{from:at,insert:"x"},selection:{anchor:at+1}});
      // Include forced DOM layout, not just state-transaction time.
      view.contentDOM.getBoundingClientRect();
      typing.push(performance.now()-start);
    }
    assert(view.state.doc.length===original.length+60,"typing lost characters");
    const scrolling:number[]=[];
    for(let i=0;i<30;i++) {
      const start=performance.now();
      const line=view.state.doc.line(Math.min(view.state.doc.lines,1+Math.floor((view.state.doc.lines-1)*i/29)));
      view.dispatch({selection:{anchor:line.from},effects:EditorView.scrollIntoView(line.from,{y:"center"})});
      await frame();view.contentDOM.getBoundingClientRect();
      scrolling.push(performance.now()-start);
    }
    const report={userAgent:navigator.userAgent,lines:view.state.doc.lines,characters:original.length,
      typing_transaction_and_layout_ms:{median:quantile(typing,.5),p95:quantile(typing,.95),max:Math.max(...typing)},
      frame_interval_ms:{median:quantile(frames,.5),p95:quantile(frames,.95)},
      scroll_jump_including_next_frame_ms:{median:quantile(scrolling,.5),p95:quantile(scrolling,.95)},
      source_preserved:true,undo_redo:true,inserted_characters:60,dialogue_indent_px:indent,
      // These are experiment budgets, not a production acceptance standard.
      typing_budget_16_7ms_p95:quantile(typing,.95)<16.7};
    load(shortScript);view.focus();
    if(window.__TAURI__) await window.__TAURI__.core.invoke("report_results",{report});
    setStatus(`Checks passed · typing p95 ${quantile(typing,.95).toFixed(1)} ms`);
    return report;
  }
  onMount(()=>{
    view=new EditorView({state:state(shortScript),parent:host});setWords(count(shortScript));
    window.trial={view,runChecks};

  });
  onCleanup(()=>view.destroy());
  function toggleTemplate(value:string) {
    setManuscript(value==="manuscript");view.dispatch({effects:mode.reconfigure(scriptLook(manuscript()))});
  }
  return <main data-theme={theme()}>
    <header class="window-title">River Writer <span>Editor trial</span></header>
    <div class="workbench">
      <nav class="activity" aria-label="Activity bar"><span class="active" title="Explorer">▣</span><span title="Search">⌕</span><span title="Source Control">⑂</span><span title="Outline">☷</span></nav>
      <aside><div class="sidebar-title">EXPLORER</div><div class="folder">⌄ RIVER TRIAL</div><div class="file selected">{manuscript()?"▤ the-other-bank.md":"▤ ferry-terminal.fountain"}</div><div class="trial-controls"><p>Editor experiment</p><button onClick={()=>{setManuscript(false);load(longScript());setStatus("Long script loaded")}}>Load long script</button><button onClick={()=>{setManuscript(true);load(prose);setStatus("Manuscript sample loaded")}}>Load manuscript sample</button><button onClick={()=>void runChecks().catch(e=>setStatus(String(e)))}>Run checks</button><button onClick={()=>setTheme(theme()==="dark"?"light":"dark")}>Switch appearance</button></div></aside>
      <section class="editor-area" classList={{manuscript:manuscript()}}>
        <div class="tabs"><div class="tab">{manuscript()?"the-other-bank.md":"ferry-terminal.fountain"}<span>×</span></div><div class="editor-actions"><select aria-label="Template" value={manuscript()?"manuscript":"screenplay"} onChange={e=>toggleTemplate(e.currentTarget.value)}><option value="screenplay">Screenplay</option><option value="manuscript">Manuscript</option></select></div></div>
        <div class="breadcrumbs">river-trial <span>›</span> {manuscript()?"the-other-bank.md":"ferry-terminal.fountain"}</div>
        <div class="editor-host" ref={host}/>
      </section>
    </div>
    <footer><span>⑂ main</span><span class="message" role="status">{status()}</span><span>{words().toLocaleString()} words</span><span>{manuscript()?"Markdown":"Fountain"}</span><span>UTF-8</span></footer>
  </main>;
}
render(()=><App/>,document.getElementById("root")!);
