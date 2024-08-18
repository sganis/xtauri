<script>
  // @ts-nocheck
      import Login from "./Login.svelte";
      import { invoke } from "@tauri-apps/api/core"
    //   import { downloadDir, appDataDir } from '@tauri-apps/api/path';
      import {FileStore, PageStore, FileViewStore, FilePageStore,
          UserStore, CurrentPath, FileRequested,JsonChanged,JsonData,JsonNewData,
          Message, Error, Progress} from '../js/store'
      import Header from "./layout/AppHeader.svelte";
      import Footer from "./layout/AppFooter.svelte";
      import AppMain from "./layout/AppMain.svelte";
  
      $: isConnected = $UserStore.isConnected && !$UserStore.isConnecting;
  
      let zoom = 1.0;
      let loginRef;

      // @ts-ignore
      const login = async (e) => {
          let args = e.detail
          console.log(args)
          $Error = "";
          
          const settings = {
              server: args.server,
              user: args.user,
              password: args.password,
              port: 22,
              private_key: "",
              home_dir: "",
          };
  
          if (args.password.length==0) {
              try {
                  $Message = "Connecting with keys...";
                  await invoke("connect_with_key", { settings: settings }); 
                  $UserStore.user = args.user;
                  $UserStore.server = args.server;
                  $UserStore.isConnected = true;
              } catch (ex) {
                  console.log(ex);
                  $UserStore.needPassword = true;
                  // @ts-ignore
                  $Error = ex;
              }
          } else {
              try {
                  $Message = "Connecting...";
                  await invoke("connect_with_password", { settings: settings }); 
                  $UserStore.user = args.user;
                  $UserStore.server = args.server;
                  $UserStore.isConnected = true;
              } catch (ex) {
                  console.log(ex);
                  $UserStore.needPassword = true;
                  // @ts-ignore
                  $Error = ex;
              }
          }
  
          if ($UserStore.isConnected) {
              if ($UserStore.needPassword) {
                  $Message = "Setting up SSH keys...";
                  try {
                      await invoke("setup_ssh", { settings: settings }); 
                      $UserStore.needPassword = false;
                  } catch (ex) {
                      console.log(ex);
                      // @ts-ignore
                      $Error = ex;
                  }
              }
              //await getFiles("/");
              //push('/files');
          }
          else {
            loginRef.focusPassword();
          }          
          $UserStore.isConnecting=false;
      }
  
      const keydown = async (e) => {
          if (e.key === '=' && e.ctrlKey) {
              zoom += 0.1;
          }
          else if (e.key === '-' && e.ctrlKey) {
              zoom -= 0.1;
          }
          else if (e.key === '0' && e.ctrlKey) {
              zoom = 1.0;
          }
          await invoke("zoom_window", {zoom});
          console.log(e)
      }
  </script>
  
  <div class="d-flex flex-column vh-100 app">
      <Header />
      {#if isConnected}
          <AppMain />
      {:else} 
          <Login on:login={login} bind:this={loginRef}/>
      {/if}
      <Footer />
  </div>
  
  <svelte:window on:keydown={keydown} />
  
  <style>
      .app {
          /* height: 100vh;
          max-height: 100vh;
          width: 100vw;
          max-width: 100vw; */
      }
  </style>
  