<script>
// @ts-nocheck

    import {invoke} from "@tauri-apps/api/core"
    import {getVersion} from '@tauri-apps/api/app';
    import {onMount} from 'svelte';
    import {UserStore} from './js/store'
    import logo from './assets/logo.png'

    let version = '';

    const appVersion = async () => {
      return await getVersion();
    }

    onMount(async () => {
      version = await appVersion();
    })

    const logout = async () => {
      try {
        const r = await invoke("disconnect");
        $UserStore.isConnected = false;
        $UserStore.needPassword = false;
        
      } catch (e) {
        console.log(e);
      }
  };


</script>

<div class="d-flex justify-content-between align-items-center header">
  <img class="logo" src={logo} alt="logo"/>
  <h2 class="title">Terminal</h2>
  <div class="flex-grow-1"></div>
  <div>v{version}</div>
  {#if $UserStore.isConnected}
    <div>{$UserStore.user}@{$UserStore.server}</div>
    <!-- svelte-ignore a11y-invalid-attribute -->
    <div><a href="#" on:click={logout}>Logout</a></div>
  {:else}
    <div></div>
  {/if}
</div>

<style>
    .title {
      white-space: nowrap;
    }
    .header {
      gap: 10px;
      padding-right: 10px;
      height: 60px;
      border-bottom: 1px solid #2b2b2b;
      background-color: black;
      color: white;
    }
    
    .logo {
      margin-left: 10px;
      
    }
    h2 {
      margin-top: 5px;
      margin-left: 10px;
      color: #868686;
    }
</style>