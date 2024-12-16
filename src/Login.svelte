<script>
  import { invoke } from "@tauri-apps/api/core"
  import {createEventDispatcher, onMount} from 'svelte'
  import {UserStore, Message, Error} from './js/store'
  import Spinner from './lib/Spinner.svelte'
  //import { sleep } from '../js/util';
  const dispatch = createEventDispatcher();

  // let server = 'localhost';
  let server = '192.168.100.202';
  let user = 'support';
  let password = '';

  /** @type {HTMLInputElement} */
  let passwordRef;
    
  onMount(async () => {
    try {
      const s = await invoke("read_settings"); 
      server = s.server;
      user = s.user;
    } catch (ex) {
      console.log('Cannot read settings: '+ex);
    }
	});

  const handleSubmit = async () => {
    $Error = '';
    $Message = "Connecting...";
    $UserStore.isConnecting = true;
    //await sleep(1000);
    dispatch('login', {server,user,password});
  }
  export const focusPassword = () => {
    setTimeout(() => {
      passwordRef.focus();
    }, 200);
  }

</script>

<div class="d-flex flex-column flex-grow-1 
      justify-content-center align-items-center login-container">
  <div class="">
    <form class="login" on:submit|preventDefault={handleSubmit} >
          <label for="server">Server</label>
          <input
            type="text"
            bind:value={server}
            disabled="{$UserStore.isConnecting}"
            id="server"
            placeholder="Enter remote ssh host name or IP address"         
          />
          <label for="user">User</label>
          <input
            type="user"
            bind:value={user}
            disabled="{$UserStore.isConnecting}"
            id="user"
            placeholder="Enter username"            
          />
          <label for="password">Password</label>
          <input
            type="password"
            bind:value={password}
            bind:this={passwordRef}
            disabled="{$UserStore.isConnecting || !$UserStore.needPassword}"
            id="password"
            placeholder="Password"            
          />
          <div class="login-button">
            <div class="w100"></div>          
            <button type="submit" class="btn btn-success" 
              disabled={$UserStore.isConnecting}>
              <i class="bi-power rp10"></i>Connect
            </button>
          </div>
        
    </form>
    <br/>
    <div class="message flex-grow-1">
      {#if $UserStore.isConnecting}
      <div class="spinner">
        <Spinner/> {$Message}
      </div>
      {:else}
      <div class="error">
        {@html $Error}
      </div>
      {/if}
    </div>
  </div>
</div>

  <style>
    .login-container {
        background-color: #1f1f1f;
    }
    .login{
        width: 400px;
        display: flex;
        flex-direction: column;  
        gap: 10px;
    }
    .login-button {
        display: flex;
        justify-content: space-between;
        padding-top: 10px;
    }
    .spinner {
        display: flex;
        justify-content:flex-start;   
        align-items: center;        
        width: 400px; 
        gap: 10px;
    }
    .message {
      width: 400px; 
      min-height: 100px;
    }
    .error {
      color: orange;
    }
    i {
      font-size: large;
      color: white;
    }
  </style>