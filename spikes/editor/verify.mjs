// Uses an installed Chrome; set up the local server with pnpm dev first.
import {mkdir,writeFile} from 'node:fs/promises';
import {chromium} from 'playwright';
const browser=await chromium.launch({headless:true,channel:"chrome"});
const page=await browser.newPage({viewport:{width:1200,height:850},colorScheme:'dark'});
const errors=[];page.on('pageerror',e=>errors.push(String(e)));page.on('response',r=>{if(r.status()>=400)errors.push(`${r.status()} ${r.url()}`)});
await page.goto('http://127.0.0.1:1420/');
await page.waitForFunction(()=>!!window.trial);
await page.evaluate(async()=>{
 for(const face of ['normal 400','normal 700','italic 400','italic 700']) {
   const loaded=await document.fonts.load(`${face} 16px "Courier Prime"`);
   if(!loaded.length)throw new Error('Courier Prime did not load');
 }
 await document.fonts.ready;
});
await mkdir(new URL('./output/',import.meta.url),{recursive:true});
const report=await page.evaluate(()=>window.trial.runChecks());
// Actual keyboard events, in addition to the deterministic transaction benchmark.
await page.locator('.cm-content').click();
await page.keyboard.press('ControlOrMeta+End');
await page.keyboard.type('A new line.');
if(!(await page.evaluate(()=>window.trial.view.state.doc.toString())).endsWith('A new line.'))throw new Error('Keyboard input lost');
await page.keyboard.press('ControlOrMeta+z');
if((await page.evaluate(()=>window.trial.view.state.doc.toString())).endsWith('A new line.'))throw new Error('Keyboard undo failed');
await page.keyboard.press('ControlOrMeta+f');
await page.locator('.cm-search input[name="search"]').fill('MAYA');
await page.keyboard.press('Escape');
await page.evaluate(()=>{const v=window.trial.view;v.dispatch({selection:{anchor:0}});v.scrollDOM.scrollTop=0;});
await page.waitForTimeout(100);
await page.screenshot({path:new URL('./output/editor-dark.png',import.meta.url).pathname});
await page.getByRole('button',{name:'Switch appearance'}).click();
await page.screenshot({path:new URL('./output/editor-light.png',import.meta.url).pathname});
const sourceBeforeSwitch=await page.evaluate(()=>window.trial.view.state.doc.toString());
await page.getByRole('combobox',{name:'Template'}).selectOption('manuscript');
if(await page.evaluate(()=>window.trial.view.state.doc.toString())!==sourceBeforeSwitch)throw new Error('Template switch modified source');
await page.getByRole('button',{name:'Load manuscript sample'}).click();
if(await page.locator('[data-kind="cue"]').count())throw new Error('Manuscript retained screenplay indents');
await page.screenshot({path:new URL('./output/manuscript.png',import.meta.url).pathname});
if(errors.length)throw new Error(errors.join('\n'));
await writeFile(new URL('./output/browser-results.json',import.meta.url),JSON.stringify({...report,keyboard_input:true,keyboard_undo:true,find:true,template_switch:true,console_errors:errors},null,2));
console.log(JSON.stringify(report,null,2));
await browser.close();
