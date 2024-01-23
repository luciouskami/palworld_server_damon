# palworld_dedicated_server_damon

## Only Support Windows

Why:

Because the Server has memory leak issues right now,so we need to restart the server before memory overflow



config.toml example:

```toml
[damon]
server_path= "E:/Program Files (x86)/Steam/steamapps/common/PalServer/PalServer.exe"
server_cli_process_name = "PalServer-Win64-Test-Cmd.exe"
memory_thresholds = 500
[server]
ip = "127.0.0.1"
port = "25575" 
password = "your admin password"
```

