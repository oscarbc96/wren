# DevTools (Servo remote debugging)

Wren can expose Servo's remote debugging server so you can inspect pages with
Firefox DevTools.

## Servo 0.8 behavior (implementation finding)

In Servo 0.8, the devtools server is started only when the browser engine is
initialized: the constellation checks `devtools_server_enabled` during startup
and starts the server once (`servo::Servo::new` / constellation setup). The
preferences `devtools_server_enabled` and `devtools_server_listen_address` can
be set through Servo's preference API (`Servo::set_preference`), but toggling
them at runtime does not start or stop the devtools server for an already
running engine. Because of that, Wren applies the DevTools setting on next
launch rather than enabling restart-free toggling in the current session.

Wren only ever listens on `127.0.0.1`.

## Settings

- `devtools.port` (default: `7000`): port for the local remote debugging
  server. The listen address is always `127.0.0.1:<port>`.

## Connecting from Firefox

1. Enable DevTools in Wren (`cmd-alt-i`). The toolbar shows that DevTools will
   apply on next launch when runtime toggling is not available.
2. Restart Wren so the setting takes effect.
3. In Firefox, open `about:debugging`.
4. Choose "This Firefox" or the setup/network location flow and add a network
   location for `localhost:7000` (or the configured port).
5. Connect and inspect the active page.
