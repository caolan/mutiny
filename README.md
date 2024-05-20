# FuckEvil

Peer-to-peer web applications runtime.

## Structure

* fe - user-friendly CLI interface to fed
* fed - long-running process to manage networking, persistence, data sync
* fes - serves an application and provides HTTP API
* lib/ - shared code for the above applications

## Usage

First, run the daemon:

```
cd fed
cargo run
```

Then, run fe to check communication between daemon and CLI:

```
cd fe
./fe ../fed/fed.socket
```

## Examples

### Ping

Demonstrates communication between frontend, fes backend, and fed daemon.

First, start the fed daemon:

```
cd fed
cargo run
```

Then, serve the app:

```
./fes/fes examples/ping
```

And open the displayed URL in your browser. You should see the message:
'Hello from fed'.
