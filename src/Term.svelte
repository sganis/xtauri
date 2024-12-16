<script>
// @ts-nocheck
    import "@xterm/xterm/css/xterm.css";
    import { Terminal }  from '@xterm/xterm';
    import { FitAddon } from "@xterm/addon-fit";
    import { invoke } from "@tauri-apps/api/core";
    import { getCurrentWindow } from '@tauri-apps/api/window'
    import {createEventDispatcher, onMount} from 'svelte';
    const dispatch = createEventDispatcher();
    let termEl;
    let term;

    onMount(async () => {
        //console.log('term mounted');
        term = new Terminal({ 
            cursorBlink: true, 
            convertEol: true,            
            fontFamily: "monospace",
            fontSize: 20,
            cols: 120,
            rows: 40,
        });

        term.write('Connecting... \n\n');
        term.write('$ ');

        let fit = new FitAddon();
        term.loadAddon(fit);
        term.open(termEl);

        term.onData(async (data) => {
            //console.log('onData:', data);
            await invoke("send_key", {key: data});
        });

        term.onLineFeed (async () => {
            // console.log('onLineFeed');          
        });
        term.onScroll(n => {
            //console.log('onScroll: ', n);
        });
        term.onSelectionChange(() => {
            //console.log('onSelectionChange');
        });
        term.onResize(async (e) => {
            //console.log('onResize', e.colos, e.rows);
            await invoke("resize", {cols: e.cols, rows: e.rows});
        });
        term.onRender (() => {
            //console.log('rendering');
            fit.fit();
        });

        let window = getCurrentWindow();
        
        window.listen("terminal-output", ({payload}) => {
            term.write(payload.data);
        });

        window.onResized(({ payload: size }) => {
            //console.log('windows resized:', size);
            fit.fit();
        })

        try {
            let r = await invoke('open_terminal');
            term.focus();   
            fit.fit();    
            console.log(r);
        } catch (e) {
            console.log('error starting terminal: ', e);
        }

    });

</script>

<div class="d-flex flex-column flex-grow-1 term-container h100">
    <div class="terminal" bind:this={termEl} />
</div>


<style>
.term-container {
    height: calc(100vh - 144px);
    /* box-sizing: border-box;   */
    background-color: #101010;
    /* margin: 0; */
    padding: 10px 10px 2px 10px;  
    /* padding-bottom: 0px; */
    /* border: 1px solid red; */
}
.terminal {
    width: 100%;
    height: 100%;
    background-color: #101010;
    /* border: 1px solid #2b2b2b; */
    padding: 0px;
    margin: 0px;
    /* padding-bottom: 200px; */
    /* margin-bottom: 100px; */
    overflow: hidden;
    /* box-sizing: border-box; */
 }
 :global(.xterm .xterm-viewport) {
    overflow-y: auto !important;  
    background-color: #101010 !important;
 }

</style>