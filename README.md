# Tauri + Svelte + Typescript

This template should help get you started developing with Tauri, Svelte and TypeScript in Vite.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).


# tunnel
# Ssh without jump host
user@Mac-Studio ~ % ssh jump
user@Mac-Studio ~ % ps aux |grep server
user              49008   0.0  0.0 410938064   6400 s005  S+    2:42PM   0:00.06 ssh user@jump

# Ssh with jump host
# One command with -J option
user@Mac-Studio ~ % ssh -J jump server
user@Mac-Studio ~ % ps aux |grep ssh
user              47753   0.3  0.0 410921680   6032 s005  S+    2:27PM   0:00.08 ssh -W [server]:22 jump
user              47752   0.1  0.0 410921680   5904 s005  S+    2:27PM   0:00.07 ssh -J jump server
user              47757   0.1  0.0 410792608   3808   ??  S     2:27PM   0:00.02 sshd-session: user@ttys003  
root             49052   0.0  0.0 410784416  11152   ??  Ss    2:27PM   0:00.05 sshd-session: user [priv]
# One command with ProxyCommand and -W option
user@Mac-Studio ~ % ssh -o ProxyCommand="ssh -W %h:%p user@jump" user@server
user@Mac-Studio ~ % ps aux |grep ssh
user              48164   0.2  0.0 411059920   6160 s005  S+    2:30PM   0:00.06 ssh -W server:22 user@jump
user              48167   0.1  0.0 410811040   4064   ??  S     2:30PM   0:00.01 sshd-session: user@ttys003  
user              48163   0.1  0.0 410930896   6256 s005  S+    2:30PM   0:00.06 ssh -o ProxyCommand=ssh -W %h:%p user@jump user@server
root             49052   0.0  0.0 410784416  11152   ??  Ss    2:30PM   0:00.05 sshd-session: user [priv]

# Two commands
user@Mac-Studio ~ % ssh -f -N -T -L 2222:server:22 user@jump
user@Mac-Studio ~ % ssh -p 2222 user@localhost   
user@Mac-Studio ~ % ps aux |grep ssh
user              48591   0.3  0.0 410940816   2688   ??  Ss    2:37PM   0:00.01 ssh -f -N -T -L 2222:server:22 user@jump
user              48594   0.3  0.0 410791168   7056 s005  S+    2:37PM   0:00.06 ssh -p 2222 user@localhost
user              48597   0.1  0.0 410810016   4064   ??  S     2:37PM   0:00.01 sshd-session: user@ttys003  
root             49052   0.0  0.0 410784416  11152   ??  Ss    2:37PM   0:00.05 sshd-session: user [priv]
