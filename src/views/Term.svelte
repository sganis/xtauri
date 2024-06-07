<script>
// @ts-nocheck

    import "@xterm/xterm/css/xterm.css";
    import { Terminal }  from '@xterm/xterm';
    import { FitAddon } from "@xterm/addon-fit";
    import { invoke } from "@tauri-apps/api/tauri";
    import { appWindow } from '@tauri-apps/api/window'
    // import { emit, listen } from '@tauri-apps/api/event';
    import {createEventDispatcher, onMount} from 'svelte';
    const dispatch = createEventDispatcher();

    
    
    let termEl;
    let term;
    let command = '';

    onMount(async () => {
        //console.log('term mounted');

        await invoke('open_terminal');
        
        term = new Terminal({ 
            cursorBlink: true, 
            convertEol: true,            
            fontFamily: "monospace",
            fontSize: 16,
        });

        term.write('Connecting... \n\n');
        term.write('$ ');

        let fit = new FitAddon();
        term.loadAddon(fit);
        term.open(termEl);
        fit.fit();

        term.onData(async data => {
            command += data;
            //if (data === '\r') {
                //term.write('\n');
            //}
            if (data === '\r') {
                term.write('\n$ ');
                try {                
                    command = command.trim();
                    if (command.length > 0) {
                        command = `sh -c "${command}"`
                        let r = await invoke('ssh_run', {command});      
                        console.log(r);      
                        term.write(r);
                        term.write('\n$ ');
                    }
                } catch(e) {
                    term.write('\x1b[1;31m ' + e + '\x1b[37m\n$ ');

                    console.log(e);
                }
                command = '';
            } else {
                term.write(data);
            }
        });
        term.onKey((key) => {
            
            console.log('onKey: ', key);
            //await invoke('send_key', key);            
        });
        term.onLineFeed (async () => {
            // console.log('onLineFeed');
            
        });
        term.onScroll(n => {
            console.log('onScroll: ', n);
        });
        term.onSelectionChange(() => {
            console.log('onSelectionChange');
        });

        appWindow.listen("send-data", (event) => {
            console.log('data: ', event)
        })
    

    });

</script>

<div class="d-flex flex-column flex-grow-1 term-container h100">
    <div class="terminal" bind:this={termEl} />
</div>


<style>
.term-container {
    /* height: calc(100vh - 144px); */
    /* box-sizing: border-box;   */
    background-color: #101010;
    margin: 0;
    padding: 10px 10px 2px 10px;  
    /* padding-bottom: 0px; */
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