# rust_simple_web_game

Project to learn Rust by creating a simple web based game. Probably not interesting to anybody but me.

Just a very rough, basic demo to understand how all the parts come together and to try some simple ideas.

Currently online at [https://rswg.lambda.nz/](https://rswg.lambda.nz/) being hosted on a $5/month VM. Pushed directly from the gh actions runner.

## Shared

Enums and a little bit of logic shared between client and server.

## Client

Browser based WASM client.

```
wasm-pack build --target web --dev
```

Builds output into pkg dir. Also has index.html.

## Server

Axum server. It hosts the static files (from symlinks to the client in the static directory). Also hosts a websocket endpoint and the actual game server loop.

- Running the server sets up the Axum /ws endpoint, ServeDir and the actual gameserver loop
- A client hits / and the ServeDir will host index.html and other assets from pkg dir
- The browser fetches the wasm and generated js bundle and initializes wasm
- The browser opens a websocket connection and joins
- The client pushes actions to and recieves updates from the game server via the socket
